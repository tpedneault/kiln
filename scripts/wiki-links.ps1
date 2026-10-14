#requires -Version 7.0
# Checks the links between wiki pages in docs/wiki/ (docs/CI.md, "The
# wiki"): every `[text](Page)` names a page that exists, and every
# `(Page#anchor)` a heading on it, using GitHub's heading slugs. Links to
# the web and links inside code are not checked. One line per broken link,
# exit 1 if there is any.
[CmdletBinding()]
param([string] $Wiki = (Join-Path (Split-Path $PSScriptRoot -Parent) 'docs/wiki'))

$ErrorActionPreference = 'Stop'

function Get-Slug([string] $heading) {
    $text = $heading -replace '`', '' -replace '\[([^\]]*)\]\([^)]*\)', '$1'
    $text = $text.Trim().ToLowerInvariant()
    $text = [regex]::Replace($text, '[^\p{L}\p{Nd}\s_-]', '')
    return ($text -replace ' ', '-')
}

# Whether $line is code: a fence line, or inside a fence. As in CommonMark,
# a fence closes only on a line of the same character, at least as long,
# so a ```` block can hold ``` examples.
function Step-Fence([string] $line) {
    if ($line -match '^\s{0,3}((`{3,})|(~{3,}))(.*)$') {
        $run = $Matches[1]
        if ($null -eq $script:fence) { $script:fence = $run; return $true }
        if ($run[0] -eq $script:fence[0] -and $run.Length -ge $script:fence.Length -and $Matches[4].Trim() -eq '') {
            $script:fence = $null
        }
        return $true
    }
    return ($null -ne $script:fence)
}

# Each page's anchors, with GitHub's -1, -2 suffixes for repeated headings.
$pages = @{}
foreach ($file in Get-ChildItem -LiteralPath $Wiki -Filter '*.md') {
    $anchors = [System.Collections.Generic.HashSet[string]]::new()
    $seen = @{}
    $script:fence = $null
    foreach ($line in Get-Content -LiteralPath $file.FullName) {
        if (Step-Fence $line) { continue }
        if ($line -match '^#{1,6}\s+(.+?)\s*#*\s*$') {
            $slug = Get-Slug $Matches[1]
            if ($seen.ContainsKey($slug)) { $seen[$slug]++; $slug = "$slug-$($seen[$slug])" } else { $seen[$slug] = 0 }
            [void] $anchors.Add($slug)
        }
    }
    $pages[$file.BaseName] = $anchors
}

$broken = 0
foreach ($file in Get-ChildItem -LiteralPath $Wiki -Filter '*.md') {
    $script:fence = $null
    $number = 0
    foreach ($line in Get-Content -LiteralPath $file.FullName) {
        $number++
        if (Step-Fence $line) { continue }
        $prose = $line -replace '`[^`]*`', ''
        foreach ($m in [regex]::Matches($prose, '\]\(([^)\s]+)\)')) {
            $target = $m.Groups[1].Value
            if ($target -match '^(https?:|mailto:)') { continue }
            $page, $anchor = $target -split '#', 2
            if ($page -eq '') { $page = $file.BaseName }
            $problem = $null
            if ($page.EndsWith('.md')) {
                $problem = "links to `"$page`": wiki links name the page without .md"
            } elseif (-not $pages.ContainsKey($page)) {
                $problem = "links to page `"$page`", which isn't in docs/wiki"
            } elseif ($anchor -and -not $pages[$page].Contains($anchor)) {
                $problem = "links to `"#$anchor`", which isn't a heading on $page"
            }
            if ($problem) {
                Write-Host "docs/wiki/$($file.Name):$number`: $problem"
                $broken++
            }
        }
    }
}
if ($broken -gt 0) {
    Write-Host "wiki: $broken broken link(s)"
    exit 1
}
Write-Host "wiki: every link resolves ($($pages.Count) pages)"
