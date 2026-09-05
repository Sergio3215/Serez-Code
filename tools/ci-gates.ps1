param(
    [ValidateSet("Full", "Core", "Ecosystem")]
    [string]$Mode = "Full",
    [string]$JsonReport = ""
)

$ErrorActionPreference = "Stop"

# Always resolve and operate from the repository root
$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $repoRoot

$isWin = if ($null -ne $IsWindows) { $IsWindows } else { $env:OS -match "Windows" }
$python = if (Get-Command python -ErrorAction SilentlyContinue) { "python" } else { "python3" }
$node = if (Get-Command node -ErrorAction SilentlyContinue) { "node" } else { "nodejs" }

function Invoke-Gate {
    param(
        [string]$Name,
        [scriptblock]$Script
    )
    Write-Host ""
    Write-Host "==================================================" -ForegroundColor Cyan
    Write-Host " [GATE] $Name" -ForegroundColor Cyan
    Write-Host "==================================================" -ForegroundColor Cyan

    $global:LASTEXITCODE = 0
    try {
        & $Script
        $code = $LASTEXITCODE
    } catch {
        Write-Host "Exception during gate execution: $_" -ForegroundColor Red
        $code = 1
    }

    if ($code -ne 0) {
        Write-Host ""
        Write-Host "CI GATE FAILED: $Name (exit code: $code)" -ForegroundColor Red
        Write-Host "No branches, tags or release state were modified." -ForegroundColor Red
        exit $code
    }
    Write-Host " [PASS] $Name" -ForegroundColor Green
}

Write-Host "Starting Serez CI Gates (Mode: $Mode, OS: $(if ($isWin) { 'Windows' } else { 'Unix' }))" -ForegroundColor Cyan

if ($Mode -eq "Full" -or $Mode -eq "Core") {
    Invoke-Gate "cargo fmt --check" {
        cargo fmt --check
    }

    Invoke-Gate "cargo check --all-targets" {
        cargo check --all-targets
    }

    Invoke-Gate "clippy baseline self-test" {
        & $python "tools/clippy_baseline.py" --self-test
    }

    Invoke-Gate "clippy baseline check" {
        & $python "tools/clippy_baseline.py" --check
    }

    Invoke-Gate "cargo test --all-targets" {
        cargo test --all-targets
    }

    Invoke-Gate "performance phases (advisory)" {
        cargo test --release --test perf_budget -- --nocapture
    }

    Invoke-Gate "Serez conformance runner" {
        if ($isWin) {
            if ($JsonReport) {
                & .\run_tests.ps1 -json $JsonReport
            } else {
                & .\run_tests.ps1
            }
        } else {
            if ($JsonReport) {
                bash ./run_tests.sh --json $JsonReport
            } else {
                bash ./run_tests.sh
            }
        }
    }

    Invoke-Gate "formatter tests (vscode-serez)" {
        Push-Location (Join-Path $repoRoot "vscode-serez")
        try {
            & $node "test/run.js"
            if ($LASTEXITCODE -ne 0) { return }
            & $node "test/provider.js"
        } finally {
            Pop-Location
        }
    }
}

if ($Mode -eq "Full" -or $Mode -eq "Ecosystem") {
    Invoke-Gate "ecosystem canary" {
        if ($isWin) {
            & .\run_ecosystem.ps1
        } else {
            if (-not (Test-Path "../serez-ui")) {
                Write-Host "Fetching pinned ecosystem packages for canary..." -ForegroundColor Cyan
                bash tools/fetch_ecosystem.sh
                if ($LASTEXITCODE -ne 0) { return }
            }
            bash ./run_ecosystem.sh
        }
    }
}

Write-Host ""
Write-Host "==================================================" -ForegroundColor Green
Write-Host " ALL CI GATES PASSED (Mode: $Mode)" -ForegroundColor Green
Write-Host "==================================================" -ForegroundColor Green
exit 0
