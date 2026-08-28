# ── Serez-Code ecosystem canary ───────────────────────────────────────────────
#
# Runs the official packages' own test suites against the core you just built,
# and reports one table. The core's own suite proves the language still does what
# its tests say; this proves the ecosystem still does what it did.
#
# `serez-ui` is the canary that matters most: it is the only official package
# exercising classes, inheritance, constructors, closures, method references,
# modules, GUI, CSS, JSX/SZX, callbacks and receiver writeback all at once, so a
# core change that breaks value semantics shows up here before anywhere else.
#
# Usage:
#   .\run_ecosystem.ps1                 # every package found next to this repo
#   .\run_ecosystem.ps1 -only serez-ui  # just one
#   .\run_ecosystem.ps1 -SkipBuild      # reuse target\release\sz.exe as-is
#
# Packages are expected as sibling checkouts (..\serez-ui, ..\serez-http, …).
# Missing ones are reported as SKIP rather than failing the run, so this works on
# a machine that has only part of the ecosystem cloned.
#
# Exit code: 0 when every package present passed, 1 otherwise.

param(
    [string] $only     = "",
    [switch] $SkipBuild
)

$ErrorActionPreference = "Stop"

$root   = Split-Path -Parent $MyInvocation.MyCommand.Path
$parent = Split-Path -Parent $root
$binary = Join-Path $root "target\release\sz.exe"

# Order matters: dependents come after what they depend on, so the first failure
# is the one closest to the core rather than a downstream symptom of it.
$packages = @(
    "serez-ui",
    "serez-http",
    "serez-ai",
    "serez-agentai",
    "serez-pack",
    "serez-apipack",
    "serez-dotenv",
    "serez-graph"
)

if ($only) { $packages = $packages | Where-Object { $_ -eq $only } }
if (-not $packages) {
    Write-Host "No package matches -only '$only'." -ForegroundColor Red
    exit 1
}

if (-not $SkipBuild) {
    Write-Host "Building core (release)..." -ForegroundColor Cyan
    $buildOut = cargo build --release 2>&1
    if ($LASTEXITCODE -ne 0) {
        Write-Host "BUILD FAILED:" -ForegroundColor Red
        $buildOut | ForEach-Object { Write-Host $_ }
        exit 1
    }
}

if (-not (Test-Path $binary)) {
    Write-Host "No binary at $binary — build the core first." -ForegroundColor Red
    exit 1
}

$version = (& $binary --version 2>&1 | Out-String).Trim()
Write-Host ""
Write-Host "Ecosystem canary against: $version" -ForegroundColor Cyan
Write-Host "Core: $binary"
Write-Host ""

$results = @()

foreach ($pkg in $packages) {
    $dir    = Join-Path $parent $pkg
    $runner = Join-Path $dir "run_tests.ps1"

    if (-not (Test-Path $dir)) {
        $results += [pscustomobject]@{ Package = $pkg; Status = "SKIP"; Detail = "not checked out next to this repo" }
        continue
    }
    if (-not (Test-Path $runner)) {
        $results += [pscustomobject]@{ Package = $pkg; Status = "SKIP"; Detail = "no run_tests.ps1" }
        continue
    }

    Write-Host "── $pkg " -NoNewline -ForegroundColor Yellow
    Write-Host ("─" * [Math]::Max(1, 60 - $pkg.Length)) -ForegroundColor Yellow

    # These runners print with Write-Host, which does not go through the
    # pipeline — the information stream (6) has to be merged in to capture it.
    Push-Location $dir
    try {
        $output = & $runner 6>&1 2>&1 | Out-String
        $code   = $LASTEXITCODE
    } catch {
        $output = $_ | Out-String
        $code   = 1
    } finally {
        Pop-Location
    }

    # Prefer the runner's own tally over its exit code: some report totals and
    # still exit 0, and a green exit with failures in the log is the worst
    # possible outcome for a canary.
    $total = [regex]::Match($output, 'TOTAL:\s*(\d+)\s*passed\s+(\d+)\s*failed')
    if ($total.Success) {
        $passed = [int] $total.Groups[1].Value
        $failed = [int] $total.Groups[2].Value
        $status = if ($failed -eq 0) { "PASS" } else { "FAIL" }
        $detail = "$passed passed, $failed failed"
    } else {
        $failLines = ([regex]::Matches($output, '(?m)^\[FAIL\]')).Count
        $status = if ($code -eq 0 -and $failLines -eq 0) { "PASS" } else { "FAIL" }
        $detail = if ($failLines -gt 0) { "$failLines failing test(s)" } else { "exit code $code" }
    }

    if ($status -eq "FAIL") {
        ($output -split "`n") |
            Where-Object { $_ -match '\[FAIL\]|ERROR|panicked' } |
            Select-Object -First 15 |
            ForEach-Object { Write-Host "    $_" -ForegroundColor Red }
    }

    Write-Host "  -> $status ($detail)" -ForegroundColor $(if ($status -eq "PASS") { "Green" } else { "Red" })
    Write-Host ""

    $results += [pscustomobject]@{ Package = $pkg; Status = $status; Detail = $detail }
}

Write-Host "═══════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "Ecosystem compatibility" -ForegroundColor Cyan
Write-Host "═══════════════════════════════════════════════" -ForegroundColor Cyan
foreach ($r in $results) {
    $color = switch ($r.Status) { "PASS" { "Green" } "FAIL" { "Red" } default { "DarkGray" } }
    Write-Host ("  {0,-16} {1,-5} {2}" -f $r.Package, $r.Status, $r.Detail) -ForegroundColor $color
}
Write-Host ""

$failed  = @($results | Where-Object { $_.Status -eq "FAIL" })
$skipped = @($results | Where-Object { $_.Status -eq "SKIP" })
$passed  = @($results | Where-Object { $_.Status -eq "PASS" })

Write-Host ("TOTAL: {0} passed  {1} failed  {2} skipped" -f $passed.Count, $failed.Count, $skipped.Count)

if ($failed.Count -gt 0) { exit 1 }
exit 0
