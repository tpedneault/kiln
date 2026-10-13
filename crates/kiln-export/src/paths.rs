//! Where file pages go, and how they link to each other and to the
//! repository.

use std::collections::HashMap;

use kiln_model::{ExportTarget, Layout};

use crate::split::Split;

/// The project-relative file of `key` in a file target.
pub fn page_file(target: &ExportTarget, split: &Split, key: &str, ext: &str) -> String {
    let base = target.path.clone().unwrap_or_default();
    if target.layout == Layout::Document {
        return base;
    }
    if key == "doc" {
        return format!("{base}/index.{ext}");
    }
    let name = |k: &str| {
        k.split_once(':')
            .map(|(_, n)| n.to_owned())
            .unwrap_or_else(|| k.to_owned())
    };
    match target.layout {
        Layout::Sections => format!("{base}/{}.{ext}", name(key)),
        _ => {
            // A folder per top-level page, a file per page below it.
            let mut top = key.to_owned();
            while let Some(parent) = split.page(&top).and_then(|p| p.parent.clone()) {
                if parent == "doc" {
                    break;
                }
                top = parent;
            }
            if top == key {
                format!("{base}/{}/index.{ext}", name(key))
            } else {
                format!("{base}/{}/{}.{ext}", name(&top), name(key))
            }
        }
    }
}

/// `to` relative to the folder of `from` (both project-relative files).
pub fn relative(from: &str, to: &str) -> String {
    let from_dir: Vec<&str> = match from.rsplit_once('/') {
        Some((d, _)) => d.split('/').filter(|s| !s.is_empty()).collect(),
        None => Vec::new(),
    };
    let to_parts: Vec<&str> = to.split('/').filter(|s| !s.is_empty()).collect();
    let common = from_dir
        .iter()
        .zip(&to_parts)
        .take_while(|(a, b)| a == b)
        .count();
    let mut parts: Vec<&str> = vec![".."; from_dir.len() - common];
    parts.extend(&to_parts[common..]);
    if parts.is_empty() {
        ".".to_owned()
    } else {
        parts.join("/")
    }
}

/// How links into the repository are written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoLinks {
    /// The export lives in the repository: relative paths, which GitLab
    /// and GitHub resolve at the revision being viewed.
    Relative,
    /// `[repository] web`, naming the last commit that changed each file
    /// (EXPORT.md, "Which commit a link names"), `fallback` for files
    /// without one (uncommitted, or past a shallow clone's boundary).
    Web {
        template: String,
        commits: HashMap<String, String>,
        fallback: String,
    },
    /// No way to link: paths are shown as text.
    None,
}

impl RepoLinks {
    /// The link to `path` (and `line`) from the page in file `from`
    /// (project-relative); `None` when there is no way to link.
    pub fn url(&self, from: &str, path: &str, line: Option<u32>) -> Option<String> {
        match self {
            RepoLinks::Relative => {
                let mut url = encode(&relative(from, path));
                if let Some(line) = line {
                    url.push_str(&format!("#L{line}"));
                }
                Some(url)
            }
            RepoLinks::Web {
                template,
                commits,
                fallback,
            } => {
                let commit = commits.get(path).unwrap_or(fallback);
                let mut url = template
                    .replace("{commit}", commit)
                    .replace("{path}", &encode(path));
                url = match line {
                    Some(l) => url.replace("{line}", &l.to_string()),
                    None => url.replace("#L{line}", "").replace("{line}", ""),
                };
                Some(url)
            }
            RepoLinks::None => None,
        }
    }

    /// The link to an image's bytes: on the web, the raw file rather than
    /// the page showing it (GitLab's `/-/blob/`, GitHub's `/blob/`).
    pub fn image_url(&self, from: &str, path: &str) -> Option<String> {
        match self {
            RepoLinks::Web {
                template,
                commits,
                fallback,
            } => {
                let raw = RepoLinks::Web {
                    template: template.replace("/blob/", "/raw/"),
                    commits: commits.clone(),
                    fallback: fallback.clone(),
                };
                raw.url(from, path, None)
            }
            other => other.url(from, path, None),
        }
    }
}

/// Percent-encodes what a URL path can't hold literally.
pub fn encode(path: &str) -> String {
    let mut out = String::new();
    for b in path.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'.' | b'-' | b'_' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_between_files() {
        assert_eq!(
            relative("docs/generated/DESIGN.md", "design/req/REQ-001.md"),
            "../../design/req/REQ-001.md"
        );
        assert_eq!(
            relative("out/spec/index.md", "out/spec/requirements.md"),
            "requirements.md"
        );
        assert_eq!(
            relative("out/spec/req/REQ-001.md", "out/spec/index.md"),
            "../index.md"
        );
        assert_eq!(relative("DESIGN.md", "src/a b.rs"), "src/a b.rs");
    }

    #[test]
    fn repository_links() {
        let web = RepoLinks::Web {
            template: "https://git/x/-/blob/{commit}/{path}#L{line}".into(),
            commits: HashMap::from([("src/a b.rs".to_owned(), "c1".to_owned())]),
            fallback: "head".into(),
        };
        assert_eq!(
            web.url("x.md", "src/a b.rs", Some(4)).unwrap(),
            "https://git/x/-/blob/c1/src/a%20b.rs#L4"
        );
        assert_eq!(
            web.url("x.md", "design/r.md", None).unwrap(),
            "https://git/x/-/blob/head/design/r.md"
        );
        assert_eq!(
            RepoLinks::Relative
                .url("docs/g/D.md", "src/a.rs", Some(2))
                .unwrap(),
            "../../src/a.rs#L2"
        );
        assert_eq!(RepoLinks::None.url("a", "b", None), None);
        assert_eq!(
            web.image_url("x.md", "img/a.png").unwrap(),
            "https://git/x/-/raw/head/img/a.png"
        );
    }
}
