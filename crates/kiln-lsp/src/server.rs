//! The protocol: initialisation, the message loop, dispatch, and
//! diagnostics. Requests are answered in the order they arrive, each after
//! the project has caught up with every edit before it; diagnostics are
//! recomputed only when no message is waiting, so a burst of keystrokes
//! costs one check.

use std::collections::HashMap;
use std::path::PathBuf;

use crossbeam_channel::{select, Receiver};
use kiln_model::{Date, Finding, Severity};
use lsp_server::{Connection, ErrorCode, Message, Notification, Request, Response};
use lsp_types::notification::{
    DidChangeTextDocument, DidChangeWatchedFiles, DidCloseTextDocument, DidOpenTextDocument,
    Notification as _, PublishDiagnostics,
};
use lsp_types::request::{
    CodeActionRequest, Completion, Formatting, GotoDefinition, HoverRequest, PrepareRenameRequest,
    References, Rename, Request as _,
};
use lsp_types::{
    CodeActionKind, CodeActionOptions, CodeActionOrCommand, CodeActionProviderCapability,
    CompletionOptions, Diagnostic, DiagnosticSeverity, InitializeParams, NumberOrString, OneOf,
    PublishDiagnosticsParams, RenameOptions, ResourceOperationKind, ServerCapabilities, ServerInfo,
    TextDocumentSyncCapability, TextDocumentSyncKind, TextDocumentSyncOptions, Uri,
};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::features::actions::ClientCan;
use crate::features::{actions, complete, hover, navigate};
use crate::text::Doc;
use crate::watch::{self, Changes, Watch};
use crate::world::World;

/// What this server offers.
pub fn capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                // Whole documents: items are small, and a full copy can't
                // drift from the editor's the way a missed range can.
                change: Some(TextDocumentSyncKind::FULL),
                ..Default::default()
            },
        )),
        completion_provider: Some(CompletionOptions {
            trigger_characters: Some(vec!["[".into(), "\"".into()]),
            ..Default::default()
        }),
        hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
        definition_provider: Some(OneOf::Left(true)),
        references_provider: Some(OneOf::Left(true)),
        rename_provider: Some(OneOf::Right(RenameOptions {
            prepare_provider: Some(true),
            work_done_progress_options: Default::default(),
        })),
        code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
            code_action_kinds: Some(vec![CodeActionKind::REFACTOR, CodeActionKind::QUICKFIX]),
            ..Default::default()
        })),
        document_formatting_provider: Some(OneOf::Left(true)),
        ..Default::default()
    }
}

struct Server {
    connection: Connection,
    world: World,
    can: ClientCan,
    /// Diagnostics last sent, by URI, so unchanged ones aren't resent.
    published: HashMap<String, Vec<Diagnostic>>,
    /// Edits arrived since diagnostics were last computed.
    dirty: bool,
    watching: Option<PathBuf>,
    watch_config: crossbeam_channel::Sender<Watch>,
    today: fn() -> Date,
}

/// Serves `connection` until the client exits.
pub fn serve(connection: Connection, today: fn() -> Date) -> Result<(), String> {
    let (id, params) = connection.initialize_start().map_err(|e| e.to_string())?;
    let params: InitializeParams =
        serde_json::from_value(params).map_err(|e| format!("initialize: {e}"))?;
    let result = lsp_types::InitializeResult {
        capabilities: capabilities(),
        server_info: Some(ServerInfo {
            name: "kiln".into(),
            version: Some(env!("CARGO_PKG_VERSION").into()),
        }),
    };
    let result = serde_json::to_value(result).map_err(|e| e.to_string())?;
    connection
        .initialize_finish(id, result)
        .map_err(|e| e.to_string())?;

    let edits = params
        .capabilities
        .workspace
        .as_ref()
        .and_then(|w| w.workspace_edit.as_ref());
    let ops = edits
        .filter(|e| e.document_changes == Some(true))
        .and_then(|e| e.resource_operations.clone())
        .unwrap_or_default();
    let can = ClientCan {
        create: ops.contains(&ResourceOperationKind::Create),
        rename: ops.contains(&ResourceOperationKind::Rename),
    };
    #[allow(deprecated)]
    let root = params
        .workspace_folders
        .as_ref()
        .and_then(|f| f.first())
        .map(|f| f.uri.clone())
        .or(params.root_uri.clone())
        .and_then(|u| crate::uri::to_path(&u));

    let (watch_config, config_rx) = crossbeam_channel::unbounded();
    let (changes_tx, changes) = crossbeam_channel::unbounded();
    let watcher = std::thread::spawn(move || watch::run(config_rx, changes_tx));
    let mut server = Server {
        connection,
        world: World::new(root.as_deref()),
        can,
        published: HashMap::new(),
        dirty: true,
        watching: None,
        watch_config,
        today,
    };
    let outcome = server.run(&changes);
    // Dropping the sender ends the watcher.
    drop(server);
    let _ = watcher.join();
    outcome
}

impl Server {
    fn run(&mut self, changes: &Receiver<Changes>) -> Result<(), String> {
        loop {
            if self.dirty && self.connection.receiver.is_empty() {
                self.dirty = false;
                self.world.sync();
                self.update_watch();
                self.publish();
            }
            select! {
                recv(self.connection.receiver) -> msg => {
                    let Ok(msg) = msg else { return Ok(()) };
                    match msg {
                        Message::Request(req) => {
                            if self.connection.handle_shutdown(&req).map_err(|e| e.to_string())? {
                                return Ok(());
                            }
                            self.request(req);
                        }
                        Message::Notification(n) => {
                            if n.method == "exit" {
                                return Ok(());
                            }
                            self.notification(n);
                        }
                        Message::Response(_) => {}
                    }
                }
                recv(changes) -> c => {
                    if let Ok(c) = c {
                        self.world.changed_on_disk(&c.paths, &c.new_code_files);
                        self.dirty = true;
                    }
                }
            }
        }
    }

    /// Tells the watcher what to watch once the project is known.
    fn update_watch(&mut self) {
        let (Some(root), Some(project)) = (&self.world.root, &self.world.project) else {
            return;
        };
        if self.watching.as_ref() == Some(root) {
            return;
        }
        self.watching = Some(root.clone());
        let _ = self.watch_config.send(Watch {
            root: root.clone(),
            item_dirs: project.config.paths.clone(),
            code_dirs: project.config.code.paths.clone(),
            code_exclude: project.config.code.exclude.clone(),
        });
    }

    fn notification(&mut self, n: Notification) {
        match n.method.as_str() {
            DidOpenTextDocument::METHOD => {
                if let Some(p) = params::<lsp_types::DidOpenTextDocumentParams>(n.params) {
                    self.world.set(&p.text_document.uri, &p.text_document.text);
                    self.dirty = true;
                }
            }
            DidChangeTextDocument::METHOD => {
                if let Some(p) = params::<lsp_types::DidChangeTextDocumentParams>(n.params) {
                    // Full sync: the last change is the whole text.
                    if let Some(change) = p.content_changes.into_iter().last() {
                        self.world.set(&p.text_document.uri, &change.text);
                        self.dirty = true;
                    }
                }
            }
            DidCloseTextDocument::METHOD => {
                if let Some(p) = params::<lsp_types::DidCloseTextDocumentParams>(n.params) {
                    self.world.close(&p.text_document.uri);
                    self.published.remove(p.text_document.uri.as_str());
                    self.send_diagnostics(p.text_document.uri, Vec::new());
                    self.dirty = true;
                }
            }
            DidChangeWatchedFiles::METHOD => {
                if let Some(p) = params::<lsp_types::DidChangeWatchedFilesParams>(n.params) {
                    let paths: Vec<String> = p
                        .changes
                        .iter()
                        .filter_map(|c| {
                            let path = crate::uri::to_path(&c.uri)?;
                            crate::uri::relative(self.world.root.as_ref()?, &path)
                        })
                        .collect();
                    self.world.changed_on_disk(&paths, &[]);
                    self.dirty = true;
                }
            }
            _ => {}
        }
    }

    fn request(&mut self, req: Request) {
        if self.world.sync() {
            self.dirty = true;
        }
        let id = req.id.clone();
        let result: Result<Value, String> = match req.method.as_str() {
            Completion::METHOD => {
                self.with_doc::<lsp_types::CompletionParams, _>(req, |s, p, _, doc, rel| {
                    let pos = p.text_document_position.position;
                    Ok(json(
                        s.world
                            .project
                            .as_ref()
                            .map(|pr| complete::complete(pr, rel, doc, pos)),
                    ))
                })
            }
            HoverRequest::METHOD => {
                self.with_doc::<lsp_types::HoverParams, _>(req, |s, p, _, doc, _| {
                    let offset = doc.offset(p.text_document_position_params.position);
                    Ok(json(
                        s.world
                            .project
                            .as_ref()
                            .and_then(|pr| hover::hover(pr, doc, offset)),
                    ))
                })
            }
            GotoDefinition::METHOD => {
                self.with_doc::<lsp_types::GotoDefinitionParams, _>(req, |s, p, _, doc, _| {
                    let offset = doc.offset(p.text_document_position_params.position);
                    Ok(json(
                        s.world
                            .project
                            .as_ref()
                            .map(|pr| navigate::definition(&s.world, pr, doc, offset)),
                    ))
                })
            }
            References::METHOD => {
                self.with_doc::<lsp_types::ReferenceParams, _>(req, |s, p, _, doc, _| {
                    let offset = doc.offset(p.text_document_position.position);
                    let declaration = p.context.include_declaration;
                    Ok(json(s.world.project.as_ref().map(|pr| {
                        navigate::references(&s.world, pr, doc, offset, declaration)
                    })))
                })
            }
            PrepareRenameRequest::METHOD => self
                .with_doc::<lsp_types::TextDocumentPositionParams, _>(req, |s, p, _, doc, _| {
                    let offset = doc.offset(p.position);
                    Ok(json(
                        s.world
                            .project
                            .as_ref()
                            .and_then(|pr| navigate::prepare_rename(pr, doc, offset)),
                    ))
                }),
            Rename::METHOD => {
                self.with_doc::<lsp_types::RenameParams, _>(req, |s, p, _, doc, rel| {
                    let offset = doc.offset(p.text_document_position.position);
                    let Some(project) = s.world.project.as_ref() else {
                        return Ok(Value::Null);
                    };
                    navigate::rename(
                        &s.world,
                        project,
                        doc,
                        rel,
                        offset,
                        &p.new_name,
                        s.can.rename,
                    )
                    .map(json)
                })
            }
            CodeActionRequest::METHOD => {
                self.with_doc::<lsp_types::CodeActionParams, _>(req, |s, p, uri, doc, rel| {
                    let offset = doc.offset(p.range.start);
                    let Some(project) = s.world.project.as_ref() else {
                        return Ok(Value::Null);
                    };
                    let actions: Vec<CodeActionOrCommand> =
                        actions::actions(&s.world, project, uri, doc, rel, offset, s.can)
                            .into_iter()
                            .map(CodeActionOrCommand::CodeAction)
                            .collect();
                    Ok(json(actions))
                })
            }
            Formatting::METHOD => {
                self.with_doc::<lsp_types::DocumentFormattingParams, _>(req, |_, _, _, doc, rel| {
                    // Only item files have front matter to format.
                    Ok(json(rel.map(|_| actions::format(doc))))
                })
            }
            other => {
                let response = Response::new_err(
                    id,
                    ErrorCode::MethodNotFound as i32,
                    format!("kiln lsp doesn't answer {other}"),
                );
                let _ = self.connection.sender.send(Message::Response(response));
                return;
            }
        };
        let response = match result {
            Ok(value) => Response {
                id,
                result: Some(value),
                error: None,
            },
            Err(message) => Response::new_err(id, ErrorCode::RequestFailed as i32, message),
        };
        let _ = self.connection.sender.send(Message::Response(response));
    }

    /// Parses `req`'s params and finds its document; a document the
    /// server hasn't seen is read from disk.
    fn with_doc<P, F>(&mut self, req: Request, f: F) -> Result<Value, String>
    where
        P: DeserializeOwned + HasDocument,
        F: FnOnce(&Server, &P, &Uri, &Doc, Option<&str>) -> Result<Value, String>,
    {
        let p: P =
            serde_json::from_value(req.params).map_err(|e| format!("{}: {e}", req.method))?;
        let uri = p.uri().clone();
        let known = self
            .world
            .open
            .get(uri.as_str())
            .map(|o| (o.doc.clone(), o.rel.clone()));
        let (doc, rel) = match known {
            Some(k) => k,
            None => {
                let path = crate::uri::to_path(&uri);
                let rel = match (&self.world.root, &path) {
                    (Some(root), Some(path)) => crate::uri::relative(root, path),
                    _ => None,
                };
                let doc = rel.as_deref().and_then(|r| self.world.doc(r));
                match doc {
                    Some(doc) => (doc, rel),
                    None => return Ok(Value::Null),
                }
            }
        };
        f(self, &p, &uri, &doc, rel.as_deref())
    }

    fn publish(&mut self) {
        let today = (self.today)();
        let by_file = self.world.findings_by_file(today);
        let mut targets: Vec<(String, String)> = self
            .world
            .open_in_project()
            .iter()
            .map(|(rel, uri)| (rel.clone(), uri.clone()))
            .collect();
        if let Some(uri) = self.world.uri(kiln_parse::CONFIG_FILE) {
            if !targets.iter().any(|(r, _)| r == kiln_parse::CONFIG_FILE) {
                targets.push((kiln_parse::CONFIG_FILE.to_owned(), uri.as_str().to_owned()));
            }
        }
        for (rel, uri) in targets {
            let Some(doc) = self.world.doc(&rel) else {
                continue;
            };
            let mut diagnostics: Vec<Diagnostic> = by_file
                .get(&rel)
                .map(|fs| fs.iter().map(|f| diagnostic(&doc, f)).collect())
                .unwrap_or_default();
            if rel == kiln_parse::CONFIG_FILE {
                if let Some(error) = &self.world.load_error {
                    diagnostics.push(Diagnostic {
                        range: doc.finding_range(1, 1),
                        severity: Some(DiagnosticSeverity::ERROR),
                        source: Some("kiln".into()),
                        message: error.clone(),
                        ..Default::default()
                    });
                }
            }
            if self.published.get(&uri) == Some(&diagnostics) {
                continue;
            }
            self.published.insert(uri.clone(), diagnostics.clone());
            if let Ok(uri) = uri.parse() {
                self.send_diagnostics(uri, diagnostics);
            }
        }
    }

    fn send_diagnostics(&self, uri: Uri, diagnostics: Vec<Diagnostic>) {
        let params = PublishDiagnosticsParams {
            uri,
            diagnostics,
            version: None,
        };
        let n = Notification::new(PublishDiagnostics::METHOD.into(), params);
        let _ = self.connection.sender.send(Message::Notification(n));
    }
}

fn diagnostic(doc: &Doc, f: &Finding) -> Diagnostic {
    Diagnostic {
        range: doc.finding_range(f.line, f.column),
        severity: Some(match f.severity {
            Severity::Error => DiagnosticSeverity::ERROR,
            Severity::Warning => DiagnosticSeverity::WARNING,
            _ => DiagnosticSeverity::INFORMATION,
        }),
        code: Some(NumberOrString::String(f.rule.code().to_owned())),
        source: Some("kiln".into()),
        message: f.message.clone(),
        ..Default::default()
    }
}

fn params<P: DeserializeOwned>(value: Value) -> Option<P> {
    serde_json::from_value(value).ok()
}

fn json<T: serde::Serialize>(value: T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

/// Request params that name a document.
trait HasDocument {
    fn uri(&self) -> &Uri;
}

macro_rules! has_document {
    ($($t:ty => $($field:ident).+;)*) => {
        $(impl HasDocument for $t {
            fn uri(&self) -> &Uri {
                &self.$($field).+.uri
            }
        })*
    };
}

has_document! {
    lsp_types::CompletionParams => text_document_position.text_document;
    lsp_types::HoverParams => text_document_position_params.text_document;
    lsp_types::GotoDefinitionParams => text_document_position_params.text_document;
    lsp_types::ReferenceParams => text_document_position.text_document;
    lsp_types::TextDocumentPositionParams => text_document;
    lsp_types::RenameParams => text_document_position.text_document;
    lsp_types::CodeActionParams => text_document;
    lsp_types::DocumentFormattingParams => text_document;
}
