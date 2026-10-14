How to get the `kiln` command onto a Windows or Linux machine, a CI
runner or a container. Kiln is one executable with no runtime, no
installer and no service. It needs no administrator rights and works
offline.

## From a release

Releases are published on the [releases
page](https://github.com/tpedneault/kiln/releases) when a version is
tagged. Each release has:

| File | For |
|---|---|
| `kiln-<version>-x86_64-pc-windows-msvc.zip` | Windows 10 and 11, 64-bit |
| `kiln-<version>-x86_64-unknown-linux-gnu.tar.gz` | Linux, 64-bit, glibc 2.35 or newer (Ubuntu 22.04, Debian 12 and newer) |
| `….sha256` | the checksum of each archive |

Each archive holds `kiln`, `kilnw`, the README, the changelog and the
licences.

If the releases page is still empty, build from source (below).

### Windows

1. Download the `.zip` and its `.sha256`, and check the archive. The hash
   should equal the first word of the `.sha256` file:

   ```powershell
   (Get-FileHash .\kiln-0.1.0-x86_64-pc-windows-msvc.zip -Algorithm SHA256).Hash.ToLower()
   Get-Content .\kiln-0.1.0-x86_64-pc-windows-msvc.zip.sha256
   ```

2. Unpack it somewhere permanent, for example
   `%LOCALAPPDATA%\Programs\kiln`.
3. Add that folder to your user `PATH`:

   ```powershell
   $dir = "$env:LOCALAPPDATA\Programs\kiln\kiln-0.1.0-x86_64-pc-windows-msvc"
   [Environment]::SetEnvironmentVariable('Path', "$([Environment]::GetEnvironmentVariable('Path','User'));$dir", 'User')
   ```

   Open a new terminal afterwards.

The two executables:

- **`kiln.exe`** is the command line. Run on its own (`kiln`) in a
  terminal, it opens the window.
- **`kilnw.exe`** opens the window with no console. Use it for desktop
  shortcuts and for Flint.

The binaries aren't signed yet, so SmartScreen may warn the first time
you run them.

### Linux

```console
$ sha256sum -c kiln-0.1.0-x86_64-unknown-linux-gnu.tar.gz.sha256
kiln-0.1.0-x86_64-unknown-linux-gnu.tar.gz: OK
$ tar -xzf kiln-0.1.0-x86_64-unknown-linux-gnu.tar.gz
$ install -m 755 kiln-0.1.0-x86_64-unknown-linux-gnu/kiln ~/.local/bin/
```

Make sure `~/.local/bin` is on your `PATH`. The binary links only the C
library, so it runs headless in CI containers. The display and GPU
libraries are loaded only when you open the window.

## From source

You need Rust stable (from [rustup](https://rustup.rs)) and git. On
Windows you also need the Visual Studio C++ Build Tools. Then:

```console
$ cargo install --locked --git https://github.com/tpedneault/kiln kiln
```

This builds and installs both `kiln` and `kilnw` into `~/.cargo/bin`,
which rustup puts on your `PATH`. The first build takes several minutes.

## Check it works

```console
$ kiln --version
kiln 0.1.0
$ kiln --help
```

Then follow the [Tutorial](Tutorial).

## Optional extras

None of these are needed to design a project. Each one enables a feature.

| For | You need |
|---|---|
| Diagrams in HTML, Word and Confluence exports | Fonts on the machine. Desktops have them. Slim Linux images may not: install `fonts-dejavu-core` (Debian, Ubuntu) or an equivalent. Markdown and Confluence targets can show the source instead ([Templates and exports](Templates-and-exports)). |
| `kiln auth confluence` and `kiln auth jira` | An OS credential store: Windows Credential Manager (built in), or a Secret Service on Linux (GNOME Keyring, KDE Wallet). Without one, set `KILN_CONFLUENCE_TOKEN` or `KILN_JIRA_TOKEN` instead. |
| Completion and navigation while editing items | An editor with a language server client. [Fenix](https://github.com/tpedneault/fenix) works out of the box ([Editors and Flint](Editors-and-Flint)). |
| Items from a launcher | [Flint](https://github.com/tpedneault/flint), with Kiln declared as a provider ([Editors and Flint](Editors-and-Flint)). |
| Baselines, and links naming commits | `git` on the `PATH`. |

## In CI

Download the release archive in the job, or build from source once and
cache it. [Continuous integration](Continuous-integration) has complete
GitHub Actions and GitLab examples.

## Updating and removing

To update, replace the executables with a newer release, or run the same
`cargo install` command again.

To remove Kiln, delete the executables. Kiln keeps only caches and
settings in its own folders, which you can delete at any time
([Configuration](Configuration)):

- Windows: `%AppData%\kiln` and `%LocalAppData%\kiln`;
- Linux: `~/.config/kiln` and `~/.local/state/kiln`.

See also: [Tutorial](Tutorial) · [Continuous
integration](Continuous-integration) · [Troubleshooting](Troubleshooting)
