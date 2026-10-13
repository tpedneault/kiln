//! The `kiln check` benchmark (DESIGN.md, "Performance"): a generated
//! project of 2,000 items and 50,000 source files, loaded and checked the
//! way `kiln check` does it.
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

// verifies: REQ-007
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
        println!("run {i}: {elapsed:.0?} ({items} items, {code_refs} code references)");
        if i > 0 {
            times.push(elapsed);
        }
    }
    times.sort();
    let median = times[times.len() / 2];
    println!("median of the warm runs: {median:.0?} (budget {BUDGET:?}, gate {GATE:?})");
    if gate && median > GATE {
        eprintln!("bench: kiln check took {median:.0?}, over the {GATE:?} gate");
        std::process::exit(1);
    }
}
