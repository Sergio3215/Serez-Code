# ── Serez-Code Test Runner ────────────────────────────────────────────────────
# Usage:
#   .\run_tests.ps1                    # run all tests
#   .\run_tests.ps1 -filter "switch"   # run tests whose name contains "switch"
#   .\run_tests.ps1 -generate          # regenerate .expected golden files
#   .\run_tests.ps1 -unit              # only run unit_*.sz tests (using framework)
#   .\run_tests.ps1 -e2e               # only run E2E tests (numbered NN_*.sz)
#
# Test types:
#   tests/NN_*.sz          → E2E tests: run and compare vs tests/NN_*.expected
#   tests/unit_*.sz        → Unit tests: prepend framework.sz, check for [FAIL]
#   tests/err_*.sz         → Error tests: must produce at least one ❌ line on stderr
#
# Exit code: 0 = all passed, 1 = failures found

param(
    [string]$filter    = "",
    [switch]$generate  = $false,
    [switch]$unit      = $false,
    [switch]$e2e       = $false
)

$ErrorActionPreference = "Stop"
$root       = $PSScriptRoot
$testsDir   = Join-Path $root "tests"
$framework  = Join-Path $testsDir "framework.sz"
$binary     = Join-Path $root "target\debug\sz.exe"
$tempFile   = Join-Path $env:TEMP "sz_test_temp.sz"

# ── Build first ───────────────────────────────────────────────────────────────
Write-Host "Building..." -ForegroundColor Cyan
Push-Location $root
$buildOut = cargo build 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "BUILD FAILED:" -ForegroundColor Red
    $buildOut | Write-Host
    exit 1
}
Pop-Location
Write-Host "Build OK`n" -ForegroundColor Green

# ── Helpers ───────────────────────────────────────────────────────────────────
$pass = 0
$fail = 0
$skip = 0

function Invoke-Sz([string]$runFile) {
    $outFile = [System.IO.Path]::GetTempFileName()
    $errFile = [System.IO.Path]::GetTempFileName()
    Start-Process -FilePath $binary -ArgumentList "`"$runFile`"" `
        -NoNewWindow -Wait `
        -RedirectStandardOutput $outFile `
        -RedirectStandardError  $errFile
    $stdout = if (Test-Path $outFile) { Get-Content $outFile } else { @() }
    $stderr = if (Test-Path $errFile) { Get-Content $errFile } else { @() }
    Remove-Item $outFile, $errFile -ErrorAction SilentlyContinue
    return @{ stdout = $stdout; stderr = $stderr }
}

function Run-Test([string]$label, [string]$file, [string]$expectedFile, [bool]$isUnit, [bool]$isErr) {
    if ($filter -and $label -notlike "*$filter*") { return }

    if ($isUnit) {
        $fw  = Get-Content $framework -Raw
        $src = Get-Content $file -Raw
        Set-Content $tempFile ($fw + "`n" + $src) -NoNewline
        $runFile = $tempFile
    } else {
        $runFile = $file
    }

    $result = Invoke-Sz $runFile
    $stdout = $result.stdout
    $stderr = $result.stderr

    if ($isErr) {
        # Error tests: must have at least one ❌ on stderr
        $hasError = ($stderr | Where-Object { $_ -match "^❌" }).Count -gt 0
        if ($hasError) {
            Write-Host "[PASS] $label" -ForegroundColor Green
            $script:pass++
        } else {
            Write-Host "[FAIL] $label — expected an error but got none" -ForegroundColor Red
            $script:fail++
        }
        return
    }

    if ($isUnit) {
        # Unit tests: look for [FAIL] lines in stdout
        $failures = $stdout | Where-Object { $_ -match "^\[FAIL\]" }
        $summary  = $stdout | Where-Object { $_ -match "^Results:" }
        if ($failures.Count -eq 0) {
            Write-Host "[PASS] $label" -ForegroundColor Green
            if ($summary) { Write-Host "       $summary" -ForegroundColor Gray }
            $script:pass++
        } else {
            Write-Host "[FAIL] $label" -ForegroundColor Red
            $failures | ForEach-Object { Write-Host "       $_" -ForegroundColor Yellow }
            $script:fail++
        }
        return
    }

    # E2E golden file test
    if ($generate) {
        $stdout | Set-Content $expectedFile
        Write-Host "[GEN]  $label → $expectedFile" -ForegroundColor Cyan
        return
    }

    if (-not (Test-Path $expectedFile)) {
        Write-Host "[SKIP] $label (no .expected file — run with -generate to create)" -ForegroundColor Yellow
        $script:skip++
        return
    }

    $expected = Get-Content $expectedFile
    $actual   = $stdout

    if ($null -eq $actual) { $actual = @() }
    if ($null -eq $expected) { $expected = @() }

    $diff = Compare-Object $expected $actual
    if ($null -eq $diff) {
        Write-Host "[PASS] $label" -ForegroundColor Green
        $script:pass++
    } else {
        Write-Host "[FAIL] $label" -ForegroundColor Red
        $diff | ForEach-Object {
            $arrow = if ($_.SideIndicator -eq "<=") { "expected:" } else { "  actual:" }
            Write-Host "       $arrow $($_.InputObject)" -ForegroundColor Yellow
        }
        $script:fail++
    }
}

# ── Discover and run tests ────────────────────────────────────────────────────
$runAll  = -not $unit -and -not $e2e

Write-Host "═══ E2E Tests ════════════════════════════════" -ForegroundColor Cyan
if ($runAll -or $e2e) {
    Get-ChildItem $testsDir -Filter "*.sz" |
        Where-Object { $_.Name -match "^\d{2}_" } |
        Sort-Object Name | ForEach-Object {
            $label    = $_.BaseName
            $expected = Join-Path $testsDir ($_.BaseName + ".expected")
            Run-Test $label $_.FullName $expected $false $false
        }
}

Write-Host ""
Write-Host "═══ Unit Tests ═══════════════════════════════" -ForegroundColor Cyan
if ($runAll -or $unit) {
    Get-ChildItem $testsDir -Filter "unit_*.sz" | Sort-Object Name | ForEach-Object {
        $label = $_.BaseName
        Run-Test $label $_.FullName "" $true $false
    }
}

Write-Host ""
Write-Host "═══ Error Tests ══════════════════════════════" -ForegroundColor Cyan
if ($runAll -or $e2e) {
    Get-ChildItem $testsDir -Filter "err_*.sz" | Sort-Object Name | ForEach-Object {
        $label = $_.BaseName
        Run-Test $label $_.FullName "" $false $true
    }
}

# ── Summary ───────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "═══════════════════════════════════════════════" -ForegroundColor Cyan
Write-Host "TOTAL: $pass passed  $fail failed  $skip skipped" -ForegroundColor $(if ($fail -gt 0) { "Red" } else { "Green" })

if ($tempFile -and (Test-Path $tempFile)) { Remove-Item $tempFile -ErrorAction SilentlyContinue }

exit $(if ($fail -gt 0) { 1 } else { 0 })
