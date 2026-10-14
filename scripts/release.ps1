#requires -Version 7.0
# The steps of a release, the same locally and in the Release workflow
# (docs/CI.md, "Releasing"). A tag `vX.Y.Z` is released only when the
# workspace version is X.Y.Z and CHANGELOG.md has a dated `## [X.Y.Z]`
# section, so `kiln --version`, the tag and the notes can't disagree.
#
#   check    the tag, the version and the changelog agree
#   notes    writes that changelog section to <OutDir>/notes.md, its
#            relative links pointed at the tag on GitHub
#   package  builds kiln and kilnw in release mode and archives them with
#            the readme, changelog and licences, plus a .sha256 file
#
# It doesn't publish anything: the workflow uploads what `package` wrote.
[CmdletBinding()]
param(
    [Parameter(Mandatory)] [ValidateSet('check', 'notes', 'package')] [string] $Step,
    [Parameter(Mandatory)] [string] $Tag,
    [string] $OutDir = 'target/dist'
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
# One line per failure, as everywhere in Kiln.
trap { [Console]::Error.WriteLine("release: $($_.Exception.Message)"); exit 1 }
$repoRoot = Split-Path $PSScriptRoot -Parent
Push-Location $repoRoot
try {
    if ($Tag -notmatch '^v(\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?)$') {
        throw "tag `"$Tag`" isn't vMAJOR.MINOR.PATCH (v0.1.0, v1.0.0-rc.1)"
    }
    $version = $Matches[1]

    # The workspace version: the first `version` under [workspace.package].
    $cargo = Get-Content Cargo.toml -Raw
    if ($cargo -notmatch '(?ms)^\[workspace\.package\]\s*$.*?^version\s*=\s*"([^"]+)"') {
        throw 'Cargo.toml has no [workspace.package] version'
    }
    if ($Matches[1] -ne $version) {
        throw "the tag says $version but Cargo.toml's [workspace.package] version is $($Matches[1]): bump it before tagging"
    }

    # The changelog section: from `## [X.Y.Z] - YYYY-MM-DD` to the next `## `.
    $lines = Get-Content CHANGELOG.md
    $heading = '^## \[' + [regex]::Escape($version) + '\] - \d{4}-\d{2}-\d{2}\s*$'
    $start = -1
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match $heading) { $start = $i; break }
    }
    if ($start -lt 0) {
        throw "CHANGELOG.md has no `"## [$version] - YYYY-MM-DD`" section: move Unreleased under it"
    }
    $end = $lines.Count
    for ($i = $start + 1; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match '^## ') { $end = $i; break }
    }
    $body = ($lines[($start + 1)..($end - 1)] -join "`n").Trim()
    # Link definitions at the bottom of the file aren't part of a section.
    $body = ($body -split "`n" | Where-Object { $_ -notmatch '^\[[^\]]+\]: ' }) -join "`n"
    if ([string]::IsNullOrWhiteSpace($body)) {
        throw "CHANGELOG.md's section for $version is empty"
    }
    # A release page resolves no relative links: point them at the tag.
    if ($cargo -notmatch '(?m)^repository\s*=\s*"([^"]+)"') {
        throw 'Cargo.toml has no [workspace.package] repository'
    }
    $blob = "$($Matches[1].TrimEnd('/'))/blob/$Tag/"
    $body = [regex]::Replace($body, '\]\((?!https?://|mailto:|#)([^)]+)\)', { param($m) "]($blob$($m.Groups[1].Value))" })

    switch ($Step) {
        'check' {
            Write-Host "release $Tag`: version and changelog agree"
        }
        'notes' {
            New-Item -ItemType Directory -Path $OutDir -Force | Out-Null
            $notes = Join-Path $OutDir 'notes.md'
            Set-Content -Path $notes -Value $body -NoNewline:$false
            Write-Host "wrote $notes"
        }
        'package' {
            cargo build --locked --release -p kiln
            if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit $LASTEXITCODE)" }
            $hostLine = rustc -vV | Where-Object { $_ -like 'host:*' }
            $target = ($hostLine -split ':\s*', 2)[1].Trim()
            $exe = if ($IsWindows) { '.exe' } else { '' }
            $name = "kiln-$version-$target"
            $stage = Join-Path $OutDir $name
            if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
            New-Item -ItemType Directory -Path $stage -Force | Out-Null
            foreach ($bin in 'kiln', 'kilnw') {
                Copy-Item "target/release/$bin$exe" $stage
            }
            foreach ($doc in 'README.md', 'CHANGELOG.md', 'LICENSE-MIT', 'LICENSE-APACHE') {
                Copy-Item $doc $stage
            }
            # Windows users open zips; tar.gz keeps the Unix executable bit.
            if ($IsWindows) {
                $archive = Join-Path $OutDir "$name.zip"
                if (Test-Path $archive) { Remove-Item $archive }
                Compress-Archive -Path $stage -DestinationPath $archive
            } else {
                $archive = Join-Path $OutDir "$name.tar.gz"
                tar -czf $archive -C $OutDir $name
                if ($LASTEXITCODE -ne 0) { throw "tar failed (exit $LASTEXITCODE)" }
            }
            $hash = (Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant()
            $leaf = Split-Path $archive -Leaf
            Set-Content -Path "$archive.sha256" -Value "$hash  $leaf"
            # The archive's own kiln must say the tagged version.
            $said = & (Join-Path $stage "kiln$exe") --version
            if ($said -ne "kiln $version") { throw "the built kiln says `"$said`", not `"kiln $version`"" }
            Write-Host "wrote $archive ($said)"
        }
    }
} finally {
    Pop-Location
}
