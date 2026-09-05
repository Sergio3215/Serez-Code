param(
    [string]$version = ""
)

# --------------------------------
# Phase 1 - Preflight
# --------------------------------
$branch = git branch --show-current
if ($branch -ne "improve") {
    Write-Host "RELEASE ABORTED: Must be on branch improve" -ForegroundColor Red
    exit 1
}

$status = git status --porcelain
if ($status) {
    Write-Host "RELEASE ABORTED: Working tree is not clean" -ForegroundColor Red
    exit 1
}

# --------------------------------
# Phase 2 - CI gates
# --------------------------------
Write-Host "Running pre-merge CI validation gates..." -ForegroundColor Cyan
.\tools\ci-gates.ps1
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

# --------------------------------
# Phase 3 - Integration & Phase 4 - Main
# --------------------------------
git switch integration
git merge improve
git switch main
git merge improve
git push origin main

# --------------------------------
# Phase 5 & 6 - Version/tag & Web
# --------------------------------
if ($version) {
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
    git add .env
    git commit -m "update version $version"
    git push --set-upstream origin main
    Pop-Location
}

# --------------------------------
# Phase 7 - Return
# --------------------------------
git switch improve

