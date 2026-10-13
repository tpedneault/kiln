//! `file:` URIs to paths and back. Written here rather than taken from a
//! URL crate because the cases that matter are few and specific: Windows
//! drive letters (`file:///C:/…`, and the `c%3A` spelling some clients
//! send), percent-encoded spaces and non-ASCII names, and `localhost` as
//! the host. Anything else (another scheme, a remote host) is not a file
//! Kiln can read, and the request for it gets an empty answer.

use std::path::{Path, PathBuf};

use lsp_types::Uri;

/// The local path a `file:` URI names.
pub fn to_path(uri: &Uri) -> Option<PathBuf> {
    let rest = uri.as_str().strip_prefix("file://")?;
    let (host, path) = rest.split_at(rest.find('/')?);
    if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") {
        return None;
    }
    let path = path.split(['?', '#']).next().unwrap_or(path);
    let decoded = percent_decode(path)?;
    Some(PathBuf::from(from_slashes(&decoded)))
}

/// The `file:` URI for an absolute path.
pub fn from_path(path: &Path) -> Option<Uri> {
    let text = path.to_string_lossy().replace('\\', "/");
    // `C:/x` → `/C:/x`; a UNC `//server/share` keeps its host.
    let text = if text.starts_with('/') {
        text
    } else {
        format!("/{text}")
    };
    let mut out = String::from("file://");
    for (i, b) in text.bytes().enumerate() {
        let keep = b.is_ascii_alphanumeric()
            || matches!(b, b'/' | b'-' | b'_' | b'.' | b'~')
            || (b == b':' && i == 2);
        if keep {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out.parse().ok()
}

fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// `/C:/x` is a Windows path `C:/x`; elsewhere a path is what it says.
fn from_slashes(path: &str) -> String {
    let b = path.as_bytes();
    if b.len() >= 3 && b[0] == b'/' && b[1].is_ascii_alphabetic() && b[2] == b':' {
        let mut p = path[1..].to_owned();
        if cfg!(windows) {
            p = p.replace('/', "\\");
        }
        p
    } else {
        path.to_owned()
    }
}

/// `path` relative to `root` with `/` separators, if it is under it.
/// Compared as text after making separators alike, case-insensitively on
/// Windows, so a drive letter the client spells `c:` still matches.
pub fn relative(root: &Path, path: &Path) -> Option<String> {
    let norm = |p: &Path| p.to_string_lossy().replace('\\', "/");
    let root = norm(root);
    let root = root.trim_end_matches('/');
    let path = norm(path);
    let same = |a: &str, b: &str| {
        if cfg!(windows) {
            a.eq_ignore_ascii_case(b)
        } else {
            a == b
        }
    };
    let head = path.get(..root.len())?;
    let rest = path.get(root.len()..)?;
    (same(head, root) && rest.starts_with('/')).then(|| rest[1..].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uri(s: &str) -> Uri {
        s.parse().unwrap()
    }

    #[test]
    fn unix_paths_round_trip_with_escapes() {
        let path = Path::new("/home/t/my project/design/REQ-001 é.md");
        let u = from_path(path).unwrap();
        assert_eq!(
            u.as_str(),
            "file:///home/t/my%20project/design/REQ-001%20%C3%A9.md"
        );
        assert_eq!(to_path(&u).unwrap(), path);
        assert_eq!(
            to_path(&uri("file://localhost/a/b.md")).unwrap(),
            Path::new("/a/b.md")
        );
        assert_eq!(to_path(&uri("file://server/a/b.md")), None);
        assert_eq!(to_path(&uri("untitled:Untitled-1")), None);
    }

    #[test]
    fn drive_letters_in_either_spelling() {
        let expected = if cfg!(windows) {
            "C:\\w\\a.md"
        } else {
            "C:/w/a.md"
        };
        assert_eq!(
            to_path(&uri("file:///C:/w/a.md")).unwrap(),
            Path::new(expected)
        );
        let lower = if cfg!(windows) {
            "c:\\w\\a.md"
        } else {
            "c:/w/a.md"
        };
        assert_eq!(
            to_path(&uri("file:///c%3A/w/a.md")).unwrap(),
            Path::new(lower)
        );
        assert_eq!(
            from_path(Path::new("C:/w/a b.md")).unwrap().as_str(),
            "file:///C:/w/a%20b.md"
        );
    }

    #[test]
    fn relative_paths_under_the_root_only() {
        assert_eq!(
            relative(Path::new("/p/"), Path::new("/p/design/a.md")).as_deref(),
            Some("design/a.md")
        );
        assert_eq!(relative(Path::new("/p"), Path::new("/pq/a.md")), None);
        assert_eq!(relative(Path::new("/p"), Path::new("/p")), None);
        if cfg!(windows) {
            assert_eq!(
                relative(Path::new("C:\\p"), Path::new("c:/p/a.md")).as_deref(),
                Some("a.md")
            );
        }
    }
}
