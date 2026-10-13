#requires -Version 7.0
# The one entry point for Kiln's gates, the same locally and in CI, on
# Windows and Linux (docs/CI.md). Kiln builds headless everywhere, so
# unlike Fenix there is one suite: every gate covers the whole workspace on
# both systems.
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
# Native exit codes are checked explicitly, including after Tee-Object.
$PSNativeCommandUseErrorActionPreference = $false
$repoRoot = Split-Path $PSScriptRoot -Parent
$runName = 'ci-{0}-{1}' -f (Get-Date -Format 'yyyyMMdd-HHmmss'), ([guid]::NewGuid().ToString('N').Substring(0, 8))
$logRoot = Join-Path $repoRoot "target/ci/$runName"
New-Item -ItemType Directory -Path $logRoot -Force | Out-Null
$results = [System.Collections.Generic.List[object]]::new()

function Invoke-Gate {
    param([string] $Name, [string] $Program, [string[]] $Arguments)
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $log = Join-Path $logRoot "$Name.log"
    Write-Host "Running $Name`: $Program $($Arguments -join ' ')"
    $code = 1
    try {
        & $Program @Arguments 2>&1 | Tee-Object -FilePath $log
        $code = $LASTEXITCODE
        if ($code -ne 0) { throw "$Name failed (exit $code). See $log" }
    } finally {
        $results.Add([ordered]@{ name = $Name; exitCode = $code; seconds = [math]::Round($timer.Elapsed.TotalSeconds, 2); log = "$Name.log" })
    }
}

Push-Location $repoRoot
try {
    Invoke-Gate 'rustc' 'rustc' @('--version', '--verbose')
    Invoke-Gate 'cargo' 'cargo' @('--version')
    Invoke-Gate 'fmt' 'cargo' @('fmt', '--all', '--check')
    # --locked prevents CI from silently choosing a different dependency graph.
    Invoke-Gate 'clippy' 'cargo' @('clippy', '--locked', '--workspace', '--all-targets', '--', '-D', 'warnings')
    Invoke-Gate 'tests' 'cargo' @('test', '--locked', '--workspace', '--no-fail-fast')
    Write-Host "All gates passed. Logs: $logRoot"
} finally {
    $results | ConvertTo-Json -Depth 4 -AsArray | Set-Content -LiteralPath (Join-Path $logRoot 'summary.json') -Encoding utf8
    Pop-Location
}
