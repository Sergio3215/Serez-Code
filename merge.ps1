param(
    [string]$version = "",
    [switch]$DryRun,
    [switch]$WaitForCI
)

$ErrorActionPreference = "Stop"

# --------------------------------
# Phase 1 - Preflight
# --------------------------------
$branch = (git branch --show-current).Trim()
if ($branch -ne "improve") {
    Write-Host "RELEASE ABORTED: Must be on branch improve (current branch: $branch)" -ForegroundColor Red
    exit 1
}

$statusPre = (git status --porcelain | Out-String).Trim()
if ($statusPre) {
    Write-Host "RELEASE ABORTED: Working tree is not clean before running gates:" -ForegroundColor Red
    Write-Host $statusPre -ForegroundColor Red
    exit 1
}

# --------------------------------
# Phase 2 - CI gates
# --------------------------------
Write-Host "Running pre-merge CI validation gates (Mode: Full)..." -ForegroundColor Cyan
pwsh ./tools/ci-gates.ps1 -Mode Full
$gateExitCode = $LASTEXITCODE
if ($gateExitCode -ne 0) {
    Write-Host ""
    Write-Host "RELEASE ABORTED: CI gates failed with exit code $gateExitCode" -ForegroundColor Red
    Write-Host "No branches, tags or release state were modified." -ForegroundColor Red
    exit $gateExitCode
}

# Post-gate working tree verification: gates must be read-only
$statusPost = (git status --porcelain | Out-String).Trim()
if ($statusPost) {
    Write-Host ""
    Write-Host "RELEASE ABORTED: Validation modified working tree:" -ForegroundColor Red
    Write-Host $statusPost -ForegroundColor Red
    Write-Host "No branches, tags or release state were modified." -ForegroundColor Red
    exit 1
}

if ($DryRun) {
    Write-Host ""
    Write-Host "DRY RUN PASSED: All pre-merge CI validation gates succeeded." -ForegroundColor Green
    Write-Host "Working tree is clean. No branches, tags or remote repositories were modified." -ForegroundColor Green
    exit 0
}

# --------------------------------
# Phase 3 - Integration & Phase 4 - Main
# --------------------------------
Write-Host "Merging improve -> integration..." -ForegroundColor Cyan
git switch integration
git merge improve
if ($LASTEXITCODE -ne 0) {
    Write-Host "Merge into integration failed" -ForegroundColor Red
    git switch improve
    exit 1
}

Write-Host "Merging improve -> main..." -ForegroundColor Cyan
git switch main
git merge improve
if ($LASTEXITCODE -ne 0) {
    Write-Host "Merge into main failed" -ForegroundColor Red
    git switch improve
    exit 1
}

Write-Host "Pushing main to origin..." -ForegroundColor Cyan
git push origin main
if ($LASTEXITCODE -ne 0) {
    Write-Host "Push to origin main failed" -ForegroundColor Red
    git switch improve
    exit 1
}

# --------------------------------
# Phase 5 & 6 - Version/tag & Web
# --------------------------------
if ($version) {
    Write-Host "Updating release tag $version..." -ForegroundColor Cyan
    git tag --delete $version 2>$null
    git tag $version
    git push origin --delete "refs/tags/$version" 2>$null
    git push origin "refs/tags/$version"

    $envPath = "..\serez-code-page\.env"
    if (Test-Path $envPath) {
        $content = Get-Content $envPath
        if ($content -match "^NEXT_PUBLIC_VERSION\s*=.*") {
            $content = $content -replace "^NEXT_PUBLIC_VERSION\s*=.*", "NEXT_PUBLIC_VERSION=$version"
        }
        else {
            $content += "NEXT_PUBLIC_VERSION=$version"
        }
    }
    else {
        $content = @("NEXT_PUBLIC_VERSION=$version")
    }
    $content | Set-Content $envPath -Encoding UTF8
    Write-Host "NEXT_PUBLIC_VERSION actualizada a $version en $envPath" -ForegroundColor Green

    Write-Host "Haciendo commit y push en serez-code-page..." -ForegroundColor Cyan
    Push-Location "..\serez-code-page"
    try {
        git add .env
        git commit -m "update version $version"
        git push --set-upstream origin main
    } finally {
        Pop-Location
    }
}

# --------------------------------
# Phase 7 - Optional Post-Push CI Wait
# --------------------------------
if ($WaitForCI) {
    Write-Host "Checking remote CI status for current commit..." -ForegroundColor Cyan
    $headSha = (git rev-parse HEAD).Trim()
    if (Get-Command gh -ErrorAction SilentlyContinue) {
        Write-Host "Waiting for GitHub Actions run for commit $headSha..." -ForegroundColor Cyan
        gh run watch --commit $headSha --exit-status
        if ($LASTEXITCODE -ne 0) {
            Write-Host "WARNING: Remote CI run failed for commit $headSha." -ForegroundColor Red
            Write-Host "No tags or branches were rolled back automatically." -ForegroundColor Yellow
        } else {
            Write-Host "Remote CI run passed successfully!" -ForegroundColor Green
        }
    } else {
        Write-Host "gh CLI not found; skipping remote CI check." -ForegroundColor Yellow
    }
}

# --------------------------------
# Phase 8 - Return
# --------------------------------
git switch improve
Write-Host "Release process completed successfully. Switched back to improve." -ForegroundColor Green
