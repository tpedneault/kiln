This page shows how to run Kiln in your CI pipeline: checking the project on every change, keeping a committed design document up to date, previewing Confluence changes on pull requests and publishing when you merge. It gives complete workflows for GitHub Actions and GitLab CI. Read it once your project has a `kiln.toml` and you want the checks to guard every merge.

## Why run Kiln in CI

Kiln keeps requirements, decisions, tests and tasks as Markdown files in your repository, linked by IDs. Those links break the way code does: someone deletes a requirement a test still verifies, two branches pick the same ID, a status is misspelt. In CI, Kiln catches that before it merges:

- **`kiln check`** fails the build on errors (a link to an item that doesn't exist, a duplicate ID, an invalid `kiln.toml`) and lists warnings (an approved requirement nothing verifies). With `--format github` or `--format gitlab`, findings appear on the lines of the pull or merge request. [Checking](Checking) lists every rule.
- **`kiln export --check`** fails when a committed export, such as `docs/generated/DESIGN.md`, is no longer what Kiln would write, so the document in the repository never drifts from the items.
- **`kiln export --dry-run`** on a pull request shows reviewers what a merge would change on Confluence, without writing anything.
- **`kiln export --yes`** on the main branch publishes, so Confluence always shows what is merged and nobody publishes by hand.

Kiln is built to run headless: the CI commands need no display, and the Linux binary runs in a plain container.

## Installing Kiln in CI

You have two ways: download a release archive, or build from source with `cargo install`.

### From a release archive

Releases are published from version tags (`v1.0.0`, …) on the [Releases page](https://github.com/tpedneault/kiln/releases). Each release has:

| File | For |
|---|---|
| `kiln-<version>-x86_64-unknown-linux-gnu.tar.gz` | Linux: `kiln` and `kilnw`, readme, changelog, licences, in a folder of the same name |
| `kiln-<version>-x86_64-pc-windows-msvc.zip` | Windows: `kiln.exe` and `kilnw.exe`, and the same documents |
| `<archive>.sha256` | the archive's SHA-256, in the format `sha256sum -c` reads |

The Linux binary needs glibc 2.35 or newer (Ubuntu 22.04, Debian 12 and later) and links nothing else beyond the C runtime. Kiln also calls `git` at run time (for commit links, `kiln diff`, `--since` and `--require-clean`), so the image needs `git`.

> **Note:** at the time of writing, no version has been tagged yet, so the Releases page may be empty. Until the first release, use [`cargo install`](#from-source-with-cargo-install). When a release exists, set `KILN_VERSION` in the examples below to its version.

Download the archive outside your checkout (in `$RUNNER_TEMP` on GitHub, `/tmp` on GitLab). Files left in the working tree count as uncommitted changes and make `kiln export --require-clean` refuse to publish.

On GitHub Actions:

```yaml
      - name: Install Kiln
        env:
          KILN_VERSION: "1.0.0"
        run: |
          name="kiln-${KILN_VERSION}-x86_64-unknown-linux-gnu"
          base="https://github.com/tpedneault/kiln/releases/download/v${KILN_VERSION}"
          cd "$RUNNER_TEMP"
          curl -fsSLO "$base/$name.tar.gz"
          curl -fsSLO "$base/$name.tar.gz.sha256"
          sha256sum -c "$name.tar.gz.sha256"
          tar -xzf "$name.tar.gz"
          echo "$RUNNER_TEMP/$name" >> "$GITHUB_PATH"
      - run: kiln --version
```

On GitLab CI, in a Debian image:

```yaml
  before_script:
    - apt-get update -qq
    - apt-get install -y -qq --no-install-recommends ca-certificates curl git
    - name="kiln-${KILN_VERSION}-x86_64-unknown-linux-gnu"
    - cd /tmp
    - curl -fsSLO "https://github.com/tpedneault/kiln/releases/download/v${KILN_VERSION}/${name}.tar.gz"
    - curl -fsSLO "https://github.com/tpedneault/kiln/releases/download/v${KILN_VERSION}/${name}.tar.gz.sha256"
    - sha256sum -c "${name}.tar.gz.sha256"
    - tar -xzf "${name}.tar.gz" -C /usr/local/bin --strip-components=1 "${name}/kiln"
    - cd "$CI_PROJECT_DIR"
    - kiln --version
```

`sha256sum -c` fails the job if the download doesn't match the published checksum. The checksum file comes from the same release; for stronger assurance, copy the expected hash into your CI file the first time and compare against that.

Faster still: build your own CI image with `kiln` already in it, and skip the download.

### From source with cargo install

This works today, with no release needed, but compiles Kiln in every job (several minutes) unless you cache Cargo's folders:

```console
$ cargo install --locked --git https://github.com/tpedneault/kiln kiln
```

This builds the latest commit of the default branch. To pin a version, add `--tag v1.0.0` (any release tag), or `--rev <commit>`. `--locked` uses Kiln's committed `Cargo.lock`. The `rust:1-bookworm` image has Rust, Cargo and `git`. On GitLab, keep `CARGO_HOME` outside the project folder (the `rust` image already does), or a Cargo cache inside the checkout makes `--require-clean` refuse.

## Full git history

Check out the full history, not a shallow clone:

- GitHub Actions: `actions/checkout` with `fetch-depth: 0`.
- GitLab CI: `GIT_DEPTH: 0` in the job's `variables`.

Why: every link an export makes into the repository (an item's source file, a code location) names **the last commit that changed that file**, not the commit being exported. That keeps published pages stable: a merge that doesn't touch a requirement doesn't create a new Confluence page version for it. Finding those commits needs the history.

In a shallow clone, an export that links to the web (a Confluence target, or an HTML file in an ignored folder) stops with:

```console
$ kiln export --dry-run
kiln: the clone is shallow: fetch full history (GitLab: GIT_DEPTH: 0; GitHub: fetch-depth: 0), or pass --allow-shallow
```

`--allow-shallow` exports anyway and links files older than the clone's boundary at the exported commit instead. A committed export (a file tracked by git, such as `docs/generated/DESIGN.md`) uses relative links and doesn't need the history.

Full history also brings the tags, which `kiln diff v1.0`, `kiln export --since v1.0` and the GUI's Changes page use as baselines.

## Secrets

Publishing to Confluence and reading Jira need tokens. Kiln reads them from environment variables in CI:

| Variable | Holds |
|---|---|
| `KILN_CONFLUENCE_TOKEN` | the Confluence token: a personal access token on Data Center, an API token on Cloud |
| `KILN_CONFLUENCE_USER` | Cloud only: the account e-mail the token belongs to (overrides `[confluence] user`) |
| `KILN_JIRA_TOKEN` | a Jira personal access token, for K207 or `kiln list --jira` (Jira Server and Data Center only) |

Rules:

- Never put a token in `kiln.toml`. `kiln check` reports anything there that looks like a secret as an error (K004).
- Kiln never writes a token to a file, a log, the plan output or an error message.
- Store tokens as CI secrets: repository or environment secrets on GitHub, masked and protected CI/CD variables on GitLab. Pass them only to the jobs that need them.
- A pull request's dry run only reads. Give it a separate token with read access if your Confluence allows it, and keep the token that can write for the publish job on the main branch.
- Don't run `kiln auth` in CI: it stores tokens in the OS credential store, which CI machines usually lack. The environment variables take precedence anyway.

`kiln check` needs no secrets at all, unless you turn on K207 (Jira status comparison). Even then, when Jira can't be read, it says `K207 not checked: …` on stderr and the other rules decide the result.

## GitHub Actions

This workflow assumes a `kiln.toml` with two export targets: `design-md`, a Markdown document committed at `docs/generated/DESIGN.md`, and `spec`, a Confluence page. Drop the jobs or steps you don't need. Save it as `.github/workflows/kiln.yml`.

```yaml
name: Kiln

on:
  push:
    branches: [main]
  pull_request:

permissions:
  contents: read

env:
  KILN_VERSION: "1.0.0"   # a release from https://github.com/tpedneault/kiln/releases

jobs:
  check:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0          # full history: links name the commit that last changed a file
      - name: Install Kiln
        run: |
          name="kiln-${KILN_VERSION}-x86_64-unknown-linux-gnu"
          base="https://github.com/tpedneault/kiln/releases/download/v${KILN_VERSION}"
          cd "$RUNNER_TEMP"
          curl -fsSLO "$base/$name.tar.gz"
          curl -fsSLO "$base/$name.tar.gz.sha256"
          sha256sum -c "$name.tar.gz.sha256"
          tar -xzf "$name.tar.gz"
          echo "$RUNNER_TEMP/$name" >> "$GITHUB_PATH"
      - name: Check the design
        run: kiln check --format github
      - name: The committed design document is current
        run: kiln export --check design-md

  plan:
    # Pull requests from forks get no secrets, so they skip the plan.
    if: github.event_name == 'pull_request' && github.event.pull_request.head.repo.full_name == github.repository
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - name: Install Kiln
        run: |
          name="kiln-${KILN_VERSION}-x86_64-unknown-linux-gnu"
          base="https://github.com/tpedneault/kiln/releases/download/v${KILN_VERSION}"
          cd "$RUNNER_TEMP"
          curl -fsSLO "$base/$name.tar.gz"
          curl -fsSLO "$base/$name.tar.gz.sha256"
          sha256sum -c "$name.tar.gz.sha256"
          tar -xzf "$name.tar.gz"
          echo "$RUNNER_TEMP/$name" >> "$GITHUB_PATH"
      - name: What a merge would publish
        env:
          KILN_CONFLUENCE_TOKEN: ${{ secrets.KILN_CONFLUENCE_TOKEN }}
          KILN_CONFLUENCE_USER: ${{ secrets.KILN_CONFLUENCE_USER }}
        run: |
          kiln export spec --dry-run
          kiln export spec --dry-run --format json > export-plan.json
      - uses: actions/upload-artifact@v4
        if: always()
        with:
          name: export-plan
          path: export-plan.json

  publish:
    if: github.event_name == 'push' && github.ref == 'refs/heads/main'
    needs: check
    runs-on: ubuntu-24.04
    # Publishes of one target must not overlap.
    concurrency:
      group: confluence
      cancel-in-progress: false
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - name: Install Kiln
        run: |
          name="kiln-${KILN_VERSION}-x86_64-unknown-linux-gnu"
          base="https://github.com/tpedneault/kiln/releases/download/v${KILN_VERSION}"
          cd "$RUNNER_TEMP"
          curl -fsSLO "$base/$name.tar.gz"
          curl -fsSLO "$base/$name.tar.gz.sha256"
          sha256sum -c "$name.tar.gz.sha256"
          tar -xzf "$name.tar.gz"
          echo "$RUNNER_TEMP/$name" >> "$GITHUB_PATH"
      - name: Publish to Confluence
        env:
          KILN_CONFLUENCE_TOKEN: ${{ secrets.KILN_CONFLUENCE_TOKEN }}
          KILN_CONFLUENCE_USER: ${{ secrets.KILN_CONFLUENCE_USER }}
        run: kiln export spec --require-clean --yes
```

What each part does:

- **`kiln check --format github`** prints findings as workflow commands (`::error file=…,line=…::`), so they annotate the changed lines. Errors fail the job (exit 1); warnings show but don't fail.
- **`kiln export --check design-md`** names the file target. Without a name, `kiln export` runs every target, Confluence included, which would need the token in this job.
- **The plan job** runs only for pull requests from the same repository. It reads Confluence and writes nothing. The text plan is in the job log; the JSON plan is uploaded as an artifact.
- **The publish job** runs on pushes to `main` after `check` passes. `--yes` is required because CI isn't a terminal: without it, Kiln stops after the plan and exits 1. `--require-clean` refuses to publish from a working tree with uncommitted or untracked files, so what is published is exactly a commit.
- **`concurrency: confluence`** runs publishes one at a time. Kiln doesn't lock Confluence pages; two overlapping publishes would plan from the same state. If they did overlap, Confluence's version conflict (409) makes the second fail safely rather than corrupt anything. A newer run waiting in the group replaces an older waiting one, which is fine: the newest `main` is what should be published.
- **Secrets:** add `KILN_CONFLUENCE_TOKEN` (and `KILN_CONFLUENCE_USER` on Cloud) as repository secrets, or as secrets of a GitHub environment that only the publish job uses.

Pin the actions to commit hashes if your policy requires it. To build Kiln instead of downloading it, replace each "Install Kiln" step with `run: cargo install --locked --git https://github.com/tpedneault/kiln kiln`.

A starting point also lives in the repository: [contrib/github-workflow.yml](https://github.com/tpedneault/kiln/blob/master/contrib/github-workflow.yml).

## GitLab CI

The same targets as above: `design-md` committed at `docs/generated/DESIGN.md`, and `spec` on Confluence. Copy the jobs you want into `.gitlab-ci.yml`.

```yaml
stages: [test, deploy]

.kiln:
  image: debian:bookworm-slim
  variables:
    GIT_DEPTH: 0              # full history: links name the commit that last changed a file
    KILN_VERSION: "1.0.0"     # a release from https://github.com/tpedneault/kiln/releases
  before_script:
    - apt-get update -qq
    - apt-get install -y -qq --no-install-recommends ca-certificates curl git
    - name="kiln-${KILN_VERSION}-x86_64-unknown-linux-gnu"
    - cd /tmp
    - curl -fsSLO "https://github.com/tpedneault/kiln/releases/download/v${KILN_VERSION}/${name}.tar.gz"
    - curl -fsSLO "https://github.com/tpedneault/kiln/releases/download/v${KILN_VERSION}/${name}.tar.gz.sha256"
    - sha256sum -c "${name}.tar.gz.sha256"
    - tar -xzf "${name}.tar.gz" -C /usr/local/bin --strip-components=1 "${name}/kiln"
    - cd "$CI_PROJECT_DIR"

kiln-check:
  extends: .kiln
  stage: test
  script:
    - kiln check --format gitlab > gl-code-quality.json || true
    - kiln check
    - kiln export --check design-md
  artifacts:
    when: always
    reports:
      codequality: gl-code-quality.json

export-plan:
  extends: .kiln
  stage: test
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
  variables:
    # A read-only token, in a masked variable that isn't protected,
    # so merge request pipelines can see it.
    KILN_CONFLUENCE_TOKEN: $KILN_CONFLUENCE_READ_TOKEN
  script:
    - kiln export spec --dry-run --format json > export-plan.json
    - kiln export spec --dry-run
  artifacts:
    when: always
    paths: [export-plan.json]

export-publish:
  extends: .kiln
  stage: deploy
  rules:
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH
  # Publishes of one target must not overlap.
  resource_group: confluence
  script:
    - kiln export spec --require-clean --yes
```

What each part does:

- **The Code Quality report.** `kiln check --format gitlab` writes a GitLab Code Quality report; `|| true` keeps the job going so the report is always saved. The second `kiln check` prints the findings in the log and fails the job on errors. GitLab shows the findings on the merge request (errors as major, warnings as minor).
- **`export-plan`** runs only in merge request pipelines and only reads Confluence. The JSON plan is kept as an artifact; the text plan is in the log.
- **`export-publish`** runs on the default branch. `resource_group: confluence` makes GitLab run one publish at a time, for the same reason as GitHub's `concurrency`.
- **Variables.** Set `KILN_CONFLUENCE_TOKEN` (and `KILN_CONFLUENCE_USER` on Cloud) as masked, protected CI/CD variables for the publish job. GitLab passes protected variables only to pipelines on protected branches and tags, and merge request pipelines usually run on unprotected branches. That is why the example gives the plan job a separate read-only token, `KILN_CONFLUENCE_READ_TOKEN`, masked but not protected, mapped to `KILN_CONFLUENCE_TOKEN` in that job only. (Variables set in the project's settings take precedence over those in `.gitlab-ci.yml`, so a merge request from a protected branch gets the protected token instead.) If you don't want a token in merge request pipelines at all, drop `export-plan`.

To build Kiln instead of downloading it, use `image: rust:1-bookworm` and replace the `before_script` with `cargo install --locked --git https://github.com/tpedneault/kiln kiln`.

A starting point also lives in the repository: [contrib/gitlab-ci.yml](https://github.com/tpedneault/kiln/blob/master/contrib/gitlab-ci.yml).

## Fonts for diagrams

Kiln draws Mermaid diagrams as pictures for Confluence, HTML and Word exports (and Markdown with `diagrams = "image"`). Drawing text needs fonts. Slim images such as `debian:bookworm-slim` have none, and the diagram library would silently draw diagrams without text, so Kiln checks first and stops with:

```text
no fonts found for diagrams: install fonts-dejavu (Debian/Ubuntu), or set diagrams = "source"
```

Fix it one of two ways:

- Install fonts in the image: add `fonts-dejavu` to the `apt-get install` line above. GitHub's hosted Ubuntu runners usually have fonts already.
- Or, for Confluence targets, set `diagrams = "source"` in `[confluence]` to show the diagram's source as a code block instead of a picture (Markdown targets draw pictures only with `[export.markdown] diagrams = "image"`). HTML and Word exports always draw pictures, so they need fonts.

Rasterised diagrams depend on the fonts a machine has, so the same export on machines with different fonts can differ in its picture bytes. Use one CI image for publishing. Picture names depend only on the diagram source, so they don't change.

Diagram pictures are cached in Kiln's state folder (`state/diagrams`). To keep the cache between runs, set `KILN_HOME` to a folder outside the checkout (for example `$RUNNER_TEMP/kiln-home`) and cache `$KILN_HOME/state/diagrams` with your CI's cache feature.

## Making unverified requirements fail CI

By default, an approved requirement that nothing verifies is a warning (rule K201), so it doesn't fail the build. To make it an error, add to `kiln.toml`:

```toml
[check]
K201 = "error"
```

K201 applies to every requirement past its type's initial status (past `draft`, with the default types) and not closed, that has no `verifies` coverage: no test item or code reference verifies it. Once raised:

```console
$ kiln check
design/requirements/REQ-003-untitled-requirement.md:2:7: K201 error: REQ-003 is approved but nothing verifies it
design/requirements/REQ-003-untitled-requirement.md:2:7: K202 warning: REQ-003 is approved but nothing implements it
kiln: 1 error, 1 warning
$ echo $?
1
```

Related rules you may raise the same way:

| Rule | Default | Finds |
|---|---|---|
| K201 | warning | a requirement past its initial status with nothing that `verifies` it |
| K202 | warning | the same with nothing that `implements` it |
| K205 | warning | status `verified` but no passing verification (a test with status `passing`, or a code reference) |
| K206 | warning | a milestone past its due date and neither done nor closed |

Any rule can also be turned off with `"off"`. [Checking](Checking) lists all of them.

## Test results and what a pull request touches

Two steps make the trace say what passed in this run and show reviewers
which requirements a change reaches (K18). Run the tests so they write a
JUnit report (for Rust, `cargo nextest run --profile ci` with JUnit
output in `.config/nextest.toml`; for Python, `pytest --junitxml=junit.xml`),
then:

```yaml
      - name: Check the design against the test results
        run: kiln check --format github --results target/nextest/ci/junit.xml
      - name: The requirements this pull request touches
        if: github.event_name == 'pull_request'
        run: kiln affected "origin/${{ github.base_ref }}" --format github
```

- **`--results`** makes a `verifies` comment count only when its test
  passed. A failed test is K216 on the comment's line, a test missing
  from the report K217, and the requirement it verified loses that
  coverage (K201). Make K201 an error (see below) and a requirement whose
  tests fail fails CI.
- **`kiln signoff`** (K19), run the same way (`kiln signoff
  "origin/${{ github.base_ref }}" --format github`), puts a warning on
  every status that is a person's call (a requirement approved, a
  decision accepted…) the pull request changed, naming the commit and
  author. Reviewers see an agent's sign-off before they merge.
- **`kiln affected`** needs the base branch in the clone: `fetch-depth:
  0` on checkout. It puts a notice on each changed line that sits under
  an `implements:` or `verifies:` comment, naming the requirement and
  the tests to re-check. It never fails the job.

## Keeping a committed design document current

A committed export is a Markdown (or HTML, or Word) document that lives in the repository, such as `docs/generated/DESIGN.md`, so people can read the design without Kiln. CI keeps it honest. Here the target writes `docs/DESIGN.md`, and an item changed since the last export:

```console
$ kiln export --check design-md
design-md (markdown, document) → docs/DESIGN.md
  would update docs/DESIGN.md
kiln: exports are out of date; run `kiln export` and commit the result
$ echo $?
1
```

When this fails, run `kiln export` locally and commit the result with the change that caused it.

Things that make it stale, and are meant to:

- any change to an item the document shows;
- code moving: code locations carry line numbers, so editing code above a `// implements:` or `// verifies:` comment changes the document too.

A committed export never names a commit (that commit doesn't exist yet when the file is written). Its links to repository files are relative, and its summary says it was generated from the files in this repository. So `--check` passes on the commit that contains the regenerated file.

See also: [Checking](Checking) · [Publishing to Confluence](Publishing-to-Confluence) · [Templates and exports](Templates-and-exports) · [Installing](Installing)
