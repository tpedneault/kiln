//! The `kiln check` and `kiln export --dry-run` benchmarks (DESIGN.md,
//! "Performance"): a generated project of 2,000 items and 50,000 source
//! files, loaded and checked the way `kiln check` does it, then exported
//! as one 2,000-item Markdown document the way `kiln export --dry-run`
//! does it (assembly, layout, references, back end, comparing with disk;
//! no diagrams, which the budget excludes); then a hundred keystrokes in
//! one item file, each re-read alone and the whole project checked again,
//! which is what the language server does per edit (its 50 ms p95
//! budget); then the Flint provider: its 2,000 static items (built, and
//! small enough for one Flint message) and its drill-down answers (the
//! 16 ms query budget).
//!
//! ```text
//! cargo run --release -p kiln-check --example bench            # report
//! cargo run --release -p kiln-check --example bench -- --gate  # CI
//! ```
//!
//! With `--gate` it exits non-zero when the median of three runs is more
//! than 50% over the 2 s budget, which is how CI catches a slowdown; the
//! margin absorbs CI machines being slower and noisier than a laptop, as
//! Flint's gate does. The budget says "cold", but the operating system's
//! file cache is warm after generating the project and can't be emptied
//! without admin rights, so this measures warm runs; the first run is
//! reported separately. Run in release: a debug build says nothing about
//! the budget.
//!
//! The project is generated once into `target/kiln-bench` and reused while
//! the generator's version is unchanged.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use kiln_index::{DiskSource, Project};
use kiln_model::{Date, Severity};

const ITEMS: usize = 2_000;
const SOURCE_FILES: usize = 50_000;
const BUDGET: Duration = Duration::from_secs(2);
const GATE: Duration = Duration::from_secs(3);
const EXPORT_BUDGET: Duration = Duration::from_secs(3);
const EXPORT_GATE: Duration = Duration::from_millis(4500);
const KEYSTROKE_BUDGET: Duration = Duration::from_millis(50);
const KEYSTROKE_GATE: Duration = Duration::from_millis(75);
const KEYSTROKES: usize = 100;
const QUERY_BUDGET: Duration = Duration::from_millis(16);
const QUERY_GATE: Duration = Duration::from_millis(24);
const QUERIES: usize = 100;
/// Bump when the generated project changes, so stale copies regenerate.
const GENERATOR: &str = "1";

/// A small deterministic generator, so every run sees the same project.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

const WORDS: [&str; 16] = [
    "telemetry",
    "frame",
    "window",
    "packet",
    "decoder",
    "render",
    "socket",
    "export",
    "parser",
    "index",
    "launcher",
    "budget",
    "latency",
    "storage",
    "config",
    "release",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/kiln-bench")
}

fn write(path: &Path, text: &str) {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).expect("create a benchmark folder");
    }
    std::fs::write(path, text).expect("write a benchmark file");
}

fn words(rng: &mut Rng, n: usize) -> String {
    (0..n)
        .map(|_| WORDS[rng.below(WORDS.len())])
        .collect::<Vec<_>>()
        .join(" ")
}

/// 1,000 requirements (a tree), 300 designs, 100 decisions, 20
/// milestones, 280 tasks and 300 tests, linked as a real project would
/// be; and source files of 40 lines, one in ten with a code reference.
fn generate(root: &Path) {
    let _ = std::fs::remove_dir_all(root);
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    write(
        &root.join("kiln.toml"),
        &format!(
            "{}\n[code]\npaths = [\"src\"]\n",
            kiln_model::default_config("Bench")
        ),
    );
    let plan: [(&str, &str, usize); 6] = [
        ("requirement", "REQ", 1000),
        ("design", "DES", 300),
        ("decision", "ADR", 100),
        ("milestone", "MS", 20),
        ("task", "TASK", 280),
        ("test", "TEST", 300),
    ];
    assert_eq!(plan.iter().map(|p| p.2).sum::<usize>(), ITEMS);
    for (ty, prefix, count) in plan {
        for n in 1..=count {
            let id = format!("{prefix}-{n:03}");
            let mut fm = format!(
                "id = \"{id}\"\ntype = \"{ty}\"\ntitle = \"{}\"\nstatus = \"{}\"\n",
                words(&mut rng, 6),
                match ty {
                    "requirement" | "design" | "test" => "draft",
                    "decision" => "accepted",
                    "milestone" => "planned",
                    _ => "todo",
                }
            );
            let req = format!("REQ-{:03}", 1 + rng.below(1000));
            match ty {
                "requirement" if n > 10 => {
                    fm.push_str(&format!("parent = \"REQ-{:03}\"\n", 1 + rng.below(n - 1)))
                }
                "design" => fm.push_str(&format!("\n[links]\nimplements = [\"{req}\"]\n")),
                "task" => fm.push_str(&format!(
                    "\n[links]\npart-of = [\"MS-{:03}\"]\nimplements = [\"{req}\"]\n",
                    1 + rng.below(20)
                )),
                "test" => fm.push_str(&format!("\n[links]\nverifies = [\"{req}\"]\n")),
                _ => {}
            }
            let body = format!(
                "{}.\n\nSee [[REQ-{:03}]] and REQ-{:03} for the {} details.\n\n## Rationale\n\n{}.\n",
                words(&mut rng, 40),
                1 + rng.below(1000),
                1 + rng.below(1000),
                words(&mut rng, 2),
                words(&mut rng, 60)
            );
            write(
                &root.join(format!("design/{ty}/{id}.md")),
                &format!("+++\n{fm}+++\n\n{body}"),
            );
        }
    }
    for n in 0..SOURCE_FILES {
        let mut text = String::new();
        for line in 0..40 {
            if line == 7 && n % 10 == 0 {
                let role = if n % 20 == 0 {
                    "verifies"
                } else {
                    "implements"
                };
                text.push_str(&format!("// {role}: REQ-{:03}\n", 1 + rng.below(1000)));
            } else {
                text.push_str(&format!(
                    "    let {} = {}({});\n",
                    WORDS[rng.below(16)],
                    WORDS[rng.below(16)],
                    line
                ));
            }
        }
        write(&root.join(format!("src/m{:02}/f{n:05}.rs", n % 100)), &text);
    }
    write(&root.join("GENERATOR"), GENERATOR);
}

fn run(root: &Path) -> (Duration, usize, usize, usize) {
    let start = Instant::now();
    let source = DiskSource::new(root);
    let project = Project::load(&source).expect("the benchmark project loads");
    let findings = kiln_check::check(
        &project,
        &source,
        Date {
            year: 2026,
            month: 1,
            day: 1,
        },
    );
    let elapsed = start.elapsed();
    let errors = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    (
        elapsed,
        project.items.len(),
        project.code_refs.len(),
        errors,
    )
}

/// `kiln export --dry-run` of the whole project as one document.
fn export(root: &Path) -> (Duration, usize) {
    let start = Instant::now();
    let source = DiskSource::new(root);
    let project = Project::load(&source).expect("the benchmark project loads");
    let findings = kiln_check::check(
        &project,
        &source,
        Date {
            year: 2026,
            month: 1,
            day: 1,
        },
    );
    let target = kiln_model::ExportTarget {
        name: "bench".into(),
        format: kiln_model::ExportFormat::Markdown,
        layout: kiln_model::Layout::Document,
        title: "Bench".into(),
        template: None,
        types: project
            .config
            .types
            .iter()
            .map(|t| t.name.clone())
            .collect(),
        exclude_status: None,
        reports: vec!["traceability".into(), "findings".into()],
        numbering: true,
        item_style: None,
        link_to: vec![],
        path: Some("out/bench.md".into()),
        markdown: Default::default(),
        html: Default::default(),
        confluence: Default::default(),
    };
    let revision = kiln_export::Revision::default();
    let assembly = kiln_export::Assembly {
        project: &project,
        findings: &findings,
        revision: &revision,
    };
    let all = vec![kiln_export::prepare(&target, None, &assembly)];
    let mut resolved = all.clone();
    kiln_export::resolve_all(&mut resolved, &all, &project);
    let files = kiln_export::render_files(
        &resolved[0],
        &resolved,
        &kiln_export::RepoLinks::Relative,
        None,
        &kiln_export::FakePictures,
        &source,
    );
    let bytes = files.iter().map(|(_, b)| b.len()).sum();
    let _plan = kiln_export::plan_files(&target, files, &source);
    (start.elapsed(), bytes)
}

/// The language server's work per keystroke: one file re-read
/// (`Project::update_file`) and every rule run again. Returns each
/// keystroke's time.
fn keystrokes(root: &Path) -> Vec<Duration> {
    let source = DiskSource::new(root);
    let mut project = Project::load(&source).expect("the benchmark project loads");
    let path = project.items[ITEMS / 2].path.clone();
    let original = std::fs::read_to_string(root.join(&path)).expect("the item file reads");
    let today = Date {
        year: 2026,
        month: 1,
        day: 1,
    };
    let mut times = Vec::with_capacity(KEYSTROKES);
    let mut typed = String::new();
    for i in 0..KEYSTROKES {
        typed.push(if i % 7 == 6 { ' ' } else { 'x' });
        let text = format!("{original}\n{typed}");
        let start = Instant::now();
        project.update_file(&path, Some(text.as_bytes()));
        let findings = kiln_check::check(&project, &source, today);
        times.push(start.elapsed());
        std::hint::black_box(findings);
    }
    times
}

/// The Flint provider: building every static item once, its encoded
/// size, and the time each drill-down answer takes (`status`, `impact`
/// of an item many depend on, `all`), in turn.
fn provider(root: &Path) -> (Duration, usize, Vec<Duration>) {
    let source = DiskSource::new(root);
    let project = Project::load(&source).expect("the benchmark project loads");
    let ctx = kiln_flint::items::Context {
        provider: "kiln",
        prefix: "k",
        root,
        confluence: true,
    };
    let start = Instant::now();
    let items = kiln_flint::items::items(&project, &ctx);
    let built = start.elapsed();
    let size = serde_json::to_vec(&items)
        .map(|b| b.len())
        .unwrap_or(usize::MAX);
    // The item most others reach: the one with the most inbound links.
    let busiest = project
        .items
        .iter()
        .max_by_key(|i| project.inbound(&i.id).len() + project.children(&i.id).len())
        .map(|i| i.id.to_string())
        .unwrap_or_default();
    let queries = [
        format!("status {busiest} "),
        format!("impact {busiest}"),
        "all 1".to_owned(),
    ];
    let mut times = Vec::with_capacity(QUERIES);
    for i in 0..QUERIES {
        let start = Instant::now();
        let found = kiln_flint::items::drilldown(&project, &queries[i % queries.len()], &ctx);
        let encoded = serde_json::to_vec(&found).map(|b| b.len()).unwrap_or(0);
        times.push(start.elapsed());
        std::hint::black_box(encoded);
    }
    (built, size, times)
}

// verifies: REQ-007, REQ-008, REQ-009, REQ-010
fn main() {
    let gate = std::env::args().any(|a| a == "--gate");
    let root = root();
    if std::fs::read_to_string(root.join("GENERATOR"))
        .ok()
        .as_deref()
        != Some(GENERATOR)
    {
        let start = Instant::now();
        generate(&root);
        println!("generated {} in {:.1?}", root.display(), start.elapsed());
    }
    let mut times = Vec::new();
    for i in 0..4 {
        let (elapsed, items, code_refs, errors) = run(&root);
        assert_eq!(errors, 0, "the generated project has no errors");
        println!(
            "run {i}: {} ms ({items} items, {code_refs} code references)",
            elapsed.as_millis()
        );
        if i > 0 {
            times.push(elapsed);
        }
    }
    times.sort();
    let median = times[times.len() / 2];
    println!(
        "median of the warm runs: {} ms (budget {} ms, gate {} ms)",
        median.as_millis(),
        BUDGET.as_millis(),
        GATE.as_millis()
    );
    let mut export_times = Vec::new();
    for i in 0..4 {
        let (elapsed, bytes) = export(&root);
        println!(
            "export run {i}: {} ms ({} KiB of Markdown)",
            elapsed.as_millis(),
            bytes / 1024
        );
        if i > 0 {
            export_times.push(elapsed);
        }
    }
    export_times.sort();
    let export_median = export_times[export_times.len() / 2];
    println!(
        "export median of the warm runs: {} ms (budget {} ms, gate {} ms)",
        export_median.as_millis(),
        EXPORT_BUDGET.as_millis(),
        EXPORT_GATE.as_millis()
    );
    let mut strokes = keystrokes(&root);
    strokes.sort();
    let p95 = strokes[strokes.len() * 95 / 100];
    println!(
        "keystroke p95 over {KEYSTROKES}: {} ms, median {} ms (budget {} ms, gate {} ms)",
        p95.as_millis(),
        strokes[strokes.len() / 2].as_millis(),
        KEYSTROKE_BUDGET.as_millis(),
        KEYSTROKE_GATE.as_millis()
    );
    let (built, size, mut queries) = provider(&root);
    queries.sort();
    let query_p95 = queries[queries.len() * 95 / 100];
    println!(
        "provider: {} items built in {} ms, {} KiB as JSON (Flint's limit {} KiB); drill-down p95 over {QUERIES}: {} ms (budget {} ms, gate {} ms)",
        ITEMS,
        built.as_millis(),
        size / 1024,
        flint_protocol::frame::MAX_BODY / 1024,
        query_p95.as_millis(),
        QUERY_BUDGET.as_millis(),
        QUERY_GATE.as_millis()
    );
    let mut failed = false;
    if gate && query_p95 > QUERY_GATE {
        eprintln!(
            "bench: a Flint drill-down took {} ms (p95), over the {} ms gate",
            query_p95.as_millis(),
            QUERY_GATE.as_millis()
        );
        failed = true;
    }
    // `provider/setItems` carries every item in one message.
    if gate && size >= flint_protocol::frame::MAX_BODY - 64 * 1024 {
        eprintln!(
            "bench: the provider's items are {} KiB, too close to Flint's {} KiB message limit",
            size / 1024,
            flint_protocol::frame::MAX_BODY / 1024
        );
        failed = true;
    }
    if gate && p95 > KEYSTROKE_GATE {
        eprintln!(
            "bench: a keystroke in the language server took {} ms (p95), over the {} ms gate",
            p95.as_millis(),
            KEYSTROKE_GATE.as_millis()
        );
        failed = true;
    }
    if gate && median > GATE {
        eprintln!(
            "bench: kiln check took {} ms, over the {} ms gate",
            median.as_millis(),
            GATE.as_millis()
        );
        failed = true;
    }
    if gate && export_median > EXPORT_GATE {
        eprintln!(
            "bench: kiln export --dry-run took {} ms, over the {} ms gate",
            export_median.as_millis(),
            EXPORT_GATE.as_millis()
        );
        failed = true;
    }
    if failed {
        std::process::exit(1);
    }
}
