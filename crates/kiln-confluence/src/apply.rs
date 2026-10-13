//! Apply (CONFLUENCE.md, "How a publish runs", step 4): carries out a plan.
//!
//! Order: every new page is created first, parents before children, with
//! a placeholder body, so children have a parent and attachments a page;
//! then each written page gets its attachments, its body (moving it when
//! its parent changed), its labels, and its `kiln` property last, with the
//! hash of the body read back (so drift detection doesn't depend on what
//! Confluence does to storage on save); then retires and prunes; the root
//! page last, so the commit it names means everything under it is at that
//! commit.
//!
//! Every op is idempotent with respect to the desired state: a publish
//! that fails half-way leaves applied ops applied, and the next run's plan
//! picks up from there.

use std::collections::{BTreeMap, HashMap};

use kiln_model::{Date, OnDrift};

use crate::client::{ClientError, Confluence, ErrorKind, Identity};
use crate::desired::DesiredPage;
use crate::model::{KilnProperty, RemotePage, Snapshot};
use crate::normalise::body_hash;
use crate::plan::{OpKind, Plan};
use crate::storage::{esc, Attachment};

/// What apply needs besides the plan.
pub struct ApplyOptions<'a> {
    pub space: &'a str,
    /// The page the root is created under (`parent`), or the adopted page.
    pub anchor: u64,
    /// The root is an existing page (`page =`).
    pub adopting: bool,
    pub project: &'a str,
    pub export: &'a str,
    /// The target's title, to tell its "Removed from the repository" page
    /// apart from another target's.
    pub target_title: &'a str,
    /// The commit being published.
    pub commit: &'a str,
    pub restrict_edits: bool,
    pub restrict_adopted: bool,
    pub on_drift: OnDrift,
    pub force: bool,
    pub today: Date,
}

/// What applying did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Applied {
    pub created: usize,
    pub updated: usize,
    pub retired: usize,
    pub pruned: usize,
    /// Pages left alone because of drift found while applying, with why.
    pub skipped: Vec<String>,
    /// Every page written: id → (version, property), for the page cache.
    pub pages: BTreeMap<u64, (u64, KilnProperty)>,
}

/// The body a page is created with, before its own is written.
pub const PLACEHOLDER: &str = "<p>Kiln is publishing this page.</p>";

/// The bytes of an attachment: a rasterised diagram or a repository file.
pub type Bytes<'a> = &'a dyn Fn(&Attachment) -> Result<Vec<u8>, String>;

struct Applier<'a> {
    client: &'a dyn Confluence,
    opts: &'a ApplyOptions<'a>,
    /// key → (page id, current version)
    pages: HashMap<String, (u64, u64)>,
    /// The remote pages as the snapshot saw them, by key.
    remote: HashMap<String, RemotePage>,
    identity: Option<Identity>,
    out: Applied,
}

/// Carries out `plan`. `bytes` gives each attachment's content; every
/// attachment the plan needs is produced before anything is written, so a
/// machine that can't draw diagrams (no fonts) fails before the first
/// call.
// implements: REQ-002
pub fn apply(
    client: &dyn Confluence,
    desired: &[DesiredPage],
    plan: &Plan,
    snapshot: &Snapshot,
    bytes: Bytes,
    opts: &ApplyOptions,
) -> Result<Applied, ClientError> {
    if !plan.refusals.is_empty() {
        return Err(ClientError::new(format!(
            "the plan was refused: {}",
            plan.refusals[0]
        )));
    }
    let kind_of = |key: &str| plan.ops.iter().find(|o| o.key == key).map(|o| o.kind);
    let writes: Vec<&DesiredPage> = desired
        .iter()
        .filter(|d| matches!(kind_of(&d.key), Some(OpKind::Create | OpKind::Update)))
        .collect();

    let mut content: HashMap<String, Vec<u8>> = HashMap::new();
    for d in &writes {
        for a in &d.attachments {
            if !content.contains_key(&a.name) {
                let data = bytes(a)
                    .map_err(|e| ClientError::new(format!("attachment {}: {e}", a.name)))?;
                content.insert(a.name.clone(), data);
            }
        }
    }

    let own = |p: &RemotePage| {
        p.property
            .as_ref()
            .is_some_and(|k| k.project == opts.project && k.export == opts.export)
    };
    let mut remote = HashMap::new();
    let mut pages = HashMap::new();
    for page in snapshot.pages() {
        if let Some(key) = page
            .property
            .as_ref()
            .filter(|_| own(page))
            .map(|k| k.key.clone())
        {
            pages.insert(key.clone(), (page.id, page.version));
            remote.insert(key, page.clone());
        }
    }
    if opts.adopting {
        if let Some(root) = &snapshot.root {
            pages.insert("doc".into(), (root.id, root.version));
            remote.entry("doc".into()).or_insert_with(|| root.clone());
        }
    }
    let identity = if opts.restrict_edits {
        Some(client.identity()?)
    } else {
        None
    };
    let mut a = Applier {
        client,
        opts,
        pages,
        remote,
        identity,
        out: Applied::default(),
    };

    // 1. Creates, parents first (desired pages come in that order).
    for d in desired
        .iter()
        .filter(|d| kind_of(&d.key) == Some(OpKind::Create))
    {
        let parent = a.parent_id(d)?;
        let created = match client.create_page(opts.space, parent, &d.title, PLACEHOLDER) {
            Ok(c) => c,
            // A publish that stopped right after creating this page left it
            // without a property: it holds the title, under the right
            // parent, with the placeholder body. It is ours to finish.
            Err(e) => match a.leftover(&d.title, parent)? {
                Some(c) => c,
                None => return Err(e),
            },
        };
        // Marked as this target's at once: if the publish stops before the
        // body is written, the next run finds the page as its own (with a
        // render hash that matches nothing) instead of as a foreign page
        // holding the title.
        let mark = KilnProperty {
            schema: 1,
            project: opts.project.to_owned(),
            export: opts.export.to_owned(),
            key: d.key.clone(),
            items: BTreeMap::new(),
            published: String::new(),
            render_hash: String::new(),
            remote_hash: body_hash(PLACEHOLDER),
            published_version: created.version,
            attachments: BTreeMap::new(),
            retired: false,
            retired_at: String::new(),
        };
        client.write_property(created.id, &mark)?;
        a.pages.insert(d.key.clone(), (created.id, created.version));
        if let Some(who) = &a.identity {
            client.restrict_edits(created.id, who)?;
        }
        a.out.created += 1;
    }

    // 2. Bodies, root last.
    let message = |key: &str| {
        plan.ops
            .iter()
            .find(|o| o.key == key)
            .map(|o| o.message.clone())
            .unwrap_or_default()
    };
    for d in writes.iter().filter(|d| d.key != "doc") {
        a.write(
            d,
            &content,
            &message(&d.key),
            kind_of(&d.key) == Some(OpKind::Update),
        )?;
    }

    // 3. Retires and prunes.
    for op in plan.ops.iter().filter(|o| o.kind == OpKind::Retire) {
        a.retire(&op.key, &op.message)?;
    }
    for op in plan.ops.iter().filter(|o| o.kind == OpKind::Prune) {
        if let Some(id) = op.page_id {
            client.trash(id)?;
            a.out.pruned += 1;
        }
    }

    // 4. The root page.
    if let Some(root) = writes.iter().find(|d| d.key == "doc") {
        a.write(
            root,
            &content,
            &message("doc"),
            kind_of("doc") == Some(OpKind::Update),
        )?;
    }
    Ok(a.out)
}

impl Applier<'_> {
    /// A page an interrupted publish created: titled `title`, under
    /// `parent`, still holding the placeholder.
    fn leftover(
        &self,
        title: &str,
        parent: u64,
    ) -> Result<Option<crate::client::Created>, ClientError> {
        let Some(id) = self.client.find_title(self.opts.space, title)? else {
            return Ok(None);
        };
        let Some(page) = self.client.page(id)? else {
            return Ok(None);
        };
        if page.parent_id == Some(parent)
            && self.client.property(id)?.is_none()
            && self.client.body(id)? == PLACEHOLDER
        {
            Ok(Some(crate::client::Created {
                id,
                version: page.version,
            }))
        } else {
            Ok(None)
        }
    }

    fn parent_id(&self, d: &DesiredPage) -> Result<u64, ClientError> {
        match &d.parent {
            None => Ok(self.opts.anchor),
            Some(key) => self.pages.get(key).map(|(id, _)| *id).ok_or_else(|| {
                ClientError::new(format!(
                    "\"{}\": its parent page ({key}) doesn't exist yet",
                    d.title
                ))
            }),
        }
    }

    /// Attachments, body, labels and property of one page.
    fn write(
        &mut self,
        d: &DesiredPage,
        content: &HashMap<String, Vec<u8>>,
        message: &str,
        updating: bool,
    ) -> Result<(), ClientError> {
        let client = self.client;
        let (id, mut version) = self.pages[&d.key];
        let old = self.remote.get(&d.key).and_then(|r| r.property.clone());

        // Attachments before the body that references them.
        let present = client.attachments(id)?;
        for a in &d.attachments {
            if !present.iter().any(|(_, name)| *name == a.name) {
                let data = content.get(&a.name).map(Vec::as_slice).unwrap_or_default();
                client.upload(id, &a.name, data, mime(&a.name))?;
            }
        }

        // The body, moving the page when its parent changed. The adopted
        // root keeps its place.
        let parent = if d.key == "doc" && self.opts.adopting {
            None
        } else {
            let wanted = self.parent_id(d)?;
            let current = self.remote.get(&d.key).and_then(|r| r.parent_id);
            (updating && current != Some(wanted)).then_some(wanted)
        };
        let mut attempts = 0;
        loop {
            attempts += 1;
            match client.update_page(id, parent, &d.title, &d.body, version + 1, message) {
                Ok(v) => {
                    version = v;
                    break;
                }
                Err(e) if e.kind == ErrorKind::Conflict && attempts == 1 => {
                    // Someone (or another publish) changed the page since the
                    // snapshot: an edit is drift; anything else is retried
                    // once on the new version.
                    let now = client.page(id)?.ok_or_else(|| {
                        ClientError::new(format!("\"{}\" disappeared while publishing", d.title))
                    })?;
                    let edited = old.as_ref().is_some_and(|p| {
                        body_hash(&client.body(id).unwrap_or_default()) != p.remote_hash
                    });
                    let overwrite = self.opts.force || self.opts.on_drift == OnDrift::Overwrite;
                    if edited && !overwrite {
                        let who = format!(
                            "edited by {} at {}, version {}",
                            now.version_author, now.version_when, now.version
                        );
                        if self.opts.on_drift == OnDrift::Skip {
                            self.out.skipped.push(format!("\"{}\": {who}", d.title));
                            return Ok(());
                        }
                        return Err(ClientError::new(format!(
                            "\"{}\" was edited in Confluence while publishing ({who}); nothing more was written to it",
                            d.title
                        )));
                    }
                    version = now.version;
                }
                Err(e) if e.kind == ErrorKind::Conflict => {
                    return Err(ClientError::new(format!(
                        "\"{}\" kept changing while publishing (409 twice): {}",
                        d.title, e.message
                    )))
                }
                Err(e) if e.kind == ErrorKind::TooLarge => {
                    return Err(ClientError::new(format!(
                        "\"{}\" ({} KiB) is too large for the server; use layout = \"sections\"",
                        d.title,
                        d.body.len() / 1024
                    )))
                }
                Err(e) => return Err(e),
            }
        }

        // Kiln's own attachments the body stopped referencing, after it.
        if let Some(old) = &old {
            for (attachment_id, name) in &present {
                let ours = old.attachments.contains_key(name);
                if ours && !d.attachments.iter().any(|a| a.name == *name) {
                    client.delete_attachment(attachment_id)?;
                }
            }
        }

        let have = client.labels(id)?;
        let missing: Vec<String> = d
            .labels
            .iter()
            .filter(|l| !have.contains(l))
            .cloned()
            .collect();
        if !missing.is_empty() {
            client.add_labels(id, &missing)?;
        }

        if updating
            && d.key == "doc"
            && self.opts.adopting
            && old.is_none()
            && self.opts.restrict_adopted
        {
            if let Some(who) = &self.identity {
                client.restrict_edits(id, who)?;
            }
        }

        // Read back what Confluence kept, and record it last.
        let remote_hash = body_hash(&client.body(id)?);
        if let Some(page) = client.page(id)? {
            version = version.max(page.version);
        }
        let property = KilnProperty {
            schema: 1,
            project: self.opts.project.to_owned(),
            export: self.opts.export.to_owned(),
            key: d.key.clone(),
            items: d.items.clone(),
            published: self.opts.commit.to_owned(),
            render_hash: d.render_hash.clone(),
            remote_hash,
            published_version: version,
            attachments: d
                .attachments
                .iter()
                .map(|a| (a.name.clone(), a.hash.clone()))
                .collect(),
            retired: false,
            retired_at: String::new(),
        };
        client.write_property(id, &property)?;
        self.pages.insert(d.key.clone(), (id, version));
        self.out.pages.insert(id, (version, property));
        if updating {
            self.out.updated += 1;
        }
        Ok(())
    }

    /// The target's "Removed from the repository" page, created under the
    /// root on first need.
    fn removed_page(&mut self) -> Result<u64, ClientError> {
        if let Some((id, _)) = self.pages.get("removed") {
            return Ok(*id);
        }
        let root = self
            .pages
            .get("doc")
            .map(|(id, _)| *id)
            .ok_or_else(|| ClientError::new("retiring pages needs the root page"))?;
        let mut title = "Removed from the repository".to_owned();
        if self.client.find_title(self.opts.space, &title)?.is_some() {
            title = format!("{title} ({})", self.opts.target_title);
        }
        let body = "<p>Pages published by Kiln whose items or sections are no longer in the repository. They stay here for reference until someone removes them (<code>kiln export --prune-removed DAYS</code>).</p>";
        let created = self
            .client
            .create_page(self.opts.space, root, &title, body)?;
        let property = KilnProperty {
            schema: 1,
            project: self.opts.project.to_owned(),
            export: self.opts.export.to_owned(),
            key: "removed".into(),
            items: BTreeMap::new(),
            published: self.opts.commit.to_owned(),
            render_hash: String::new(),
            remote_hash: body_hash(body),
            published_version: created.version,
            attachments: BTreeMap::new(),
            retired: false,
            retired_at: String::new(),
        };
        self.client.write_property(created.id, &property)?;
        self.pages
            .insert("removed".into(), (created.id, created.version));
        Ok(created.id)
    }

    /// Moves a page nothing produces any more under "Removed from the
    /// repository", with a banner over its last published content.
    fn retire(&mut self, key: &str, message: &str) -> Result<(), ClientError> {
        let Some(page) = self.remote.get(key).cloned() else {
            return Ok(());
        };
        let removed = self.removed_page()?;
        let client = self.client;
        let old_body = client.body(page.id)?;
        let short = &self.opts.commit[..self.opts.commit.len().min(12)];
        let banner = format!(
            "<ac:structured-macro ac:name=\"warning\"><ac:rich-text-body><p>{}</p></ac:rich-text-body></ac:structured-macro>",
            esc(&format!(
                "Removed from the repository at {short}: this page is no longer published and isn't kept up to date. Its last published content follows."
            ))
        );
        let version = client.update_page(
            page.id,
            Some(removed),
            &page.title,
            &format!("{banner}{old_body}"),
            page.version + 1,
            message,
        )?;
        client.add_labels(page.id, &["kiln-removed".to_owned()])?;
        let mut property = page.property.clone().unwrap_or_else(|| KilnProperty {
            schema: 1,
            project: self.opts.project.to_owned(),
            export: self.opts.export.to_owned(),
            key: key.to_owned(),
            items: BTreeMap::new(),
            published: String::new(),
            render_hash: String::new(),
            remote_hash: String::new(),
            published_version: 0,
            attachments: BTreeMap::new(),
            retired: false,
            retired_at: String::new(),
        });
        property.retired = true;
        property.retired_at = self.opts.today.to_string();
        property.remote_hash = body_hash(&client.body(page.id)?);
        property.published_version = version;
        property.render_hash = String::new();
        client.write_property(page.id, &property)?;
        self.out.pages.insert(page.id, (version, property));
        self.out.retired += 1;
        Ok(())
    }
}

fn mime(name: &str) -> &'static str {
    match name
        .rsplit('.')
        .next()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}
