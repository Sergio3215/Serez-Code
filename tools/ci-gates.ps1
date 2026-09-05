function Invoke-Gate {
    param([string]$Name, [scriptblock]$Script)
    Write-Host "Running gate: $Name" -ForegroundColor Cyan
    & $Script
    $exitCode = $LASTEXITCODE
    if ($exitCode -ne 0) {
        Write-Host ""
        Write-Host "RELEASE ABORTED" -ForegroundColor Red
        Write-Host "CI gate failed: $Name" -ForegroundColor Red
        Write-Host "exit code: $exitCode" -ForegroundColor Red
        Write-Host "No branches, tags or release state were modified." -ForegroundColor Red
        exit $exitCode
    }
}

Invoke-Gate "cargo fmt" { cargo fmt --check }
Invoke-Gate "cargo check" { cargo check --all-targets }
Invoke-Gate "clippy baseline check" { python tools/clippy_baseline.py --check }
Invoke-Gate "cargo test" { cargo test --all-targets }
Invoke-Gate "runner Serez" { .\run_tests.ps1 }
Invoke-Gate "ecosystem canary" { .\run_ecosystem.ps1 }

# formatter tests
Push-Location vscode-serez
Invoke-Gate "formatter tests" { npm test }
Pop-Location

