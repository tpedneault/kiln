//! Export targets (docs/EXPORT.md): assembly into one numbered document
//! with an anchor per item, from a template's `<!-- kiln:… -->` directives
//! or one section per type; the three layouts (`document`, `sections`,
//! `tree`) as splits of that one document with references resolved across
//! pages; diagram rasterising through `fenix-diagram` and its cache; and
//! the Markdown, HTML, Word and Confluence wiki markup back ends with
//! their file destination.
//!
//! Every target runs the same pipeline (select and assemble, split,
//! resolve, back end, destination), so a layout or a format is added once
//! and works with every other choice. `document` is the default; the
//! other layouts are splits of it, never separate code paths.
//!
//! Pure except for writing files ([`files::apply`]) and the diagram
//! cache. It deliberately knows nothing about Confluence: that back end
//! and its destination live in `kiln-confluence`, which takes the split
//! pages from here. Facts from outside (the commit, each file's last
//! commit, whether the output is inside the repository) come in as data.

mod assemble;
mod document;
mod docx;
pub mod files;
mod html;
mod markdown;
mod paths;
mod pictures;
mod resolve;
mod split;
mod wiki;

pub use assemble::{assemble, capitalised, short, Assembled, Assembly, Revision};
pub use document::{toc_levels, Document, Node, Section, SectionKind};
pub use docx::{bookmark as docx_bookmark, Reference as DocxReference};
pub use files::{apply, plan_files, FilePlan, MANIFEST};
pub use markdown::{escape as markdown_escape, github_slug, gitlab_slug, FileContext};
pub use paths::{encode, page_file, relative, RepoLinks};
pub use pictures::{
    picture_name, FakePictures, FenixPictures, Picture, Pictures, FENIX_DIAGRAM_REV, NO_FONTS,
};
pub use resolve::{resolve_page, Links, Locations};
pub use split::{split, Location, Page, Split};

use kiln_index::FileSource;
use kiln_model::{ExportFormat, ExportTarget};

/// One target, assembled and split, with its references not yet resolved.
#[derive(Debug, Clone)]
pub struct Prepared {
    pub target: ExportTarget,
    pub split: Split,
    pub left_out: Vec<kiln_model::ItemId>,
}

/// Assembles and splits `target`. `template` is its template's text.
// implements: REQ-010
pub fn prepare(target: &ExportTarget, template: Option<&str>, assembly: &Assembly) -> Prepared {
    let assembled = assemble(target, template, assembly);
    Prepared {
        target: target.clone(),
        split: split(assembled.document, target.layout),
        left_out: assembled.left_out,
    }
}

/// Whether `a` can link into `b`: files link to files, Confluence pages
/// to Confluence pages, and wiki markup only to wiki markup, whose links
/// name pages by title.
fn same_family(a: ExportFormat, b: ExportFormat) -> bool {
    let wiki = |f| f == ExportFormat::ConfluenceWiki;
    a.is_file() == b.is_file() && wiki(a) == wiki(b)
}

/// Resolves the references of every prepared target, through each one's
/// `link_to` targets among `all`.
pub fn resolve_all(prepared: &mut [Prepared], all: &[Prepared], project: &kiln_index::Project) {
    for p in prepared.iter_mut() {
        let own = p.split.locations.clone();
        let others: Vec<(String, &Locations)> = p
            .target
            .link_to
            .iter()
            .filter_map(|name| all.iter().find(|o| &o.target.name == name))
            .filter(|o| same_family(p.target.format, o.target.format))
            .map(|o| (o.target.name.clone(), &o.split.locations))
            .collect();
        let links = Links {
            project,
            own: &own,
            others,
        };
        for page in &mut p.split.pages {
            resolve_page(page, &links);
        }
    }
}

/// The files of a resolved file target.
pub fn render_files(
    p: &Prepared,
    all: &[Prepared],
    repo: &RepoLinks,
    jira_url: Option<&str>,
    pictures: &dyn Pictures,
    files: &dyn FileSource,
) -> Vec<(String, Vec<u8>)> {
    let others = p
        .target
        .link_to
        .iter()
        .filter_map(|name| all.iter().find(|o| &o.target.name == name))
        .filter(|o| o.target.format.is_file())
        .map(|o| (o.target.name.clone(), &o.target, &o.split))
        .collect();
    let ctx = FileContext {
        target: &p.target,
        split: &p.split,
        others,
        repo,
        jira_url,
        pictures,
    };
    match p.target.format {
        ExportFormat::Markdown => markdown::render(&ctx),
        ExportFormat::Html => html::render(&ctx, files),
        ExportFormat::Docx => docx::render(&ctx, files),
        ExportFormat::ConfluenceWiki => wiki::render(&ctx, files),
        ExportFormat::Confluence => Vec::new(),
    }
}

#[cfg(test)]
mod tests;
