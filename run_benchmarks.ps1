# ── Serez-Code Advanced Benchmark Suite ──────────────────────────────────────
# Usage:
#   .\run_benchmarks.ps1              # build + run all benchmarks (5 iterations)
#   .\run_benchmarks.ps1 -NoBuild     # skip cargo build (binary must already exist)
#   .\run_benchmarks.ps1 -N 10        # 10 warm-up+timed iterations
#   .\run_benchmarks.ps1 -Filter oop  # only benchmarks whose name contains "oop"
#   .\run_benchmarks.ps1 -Json b.json # also write a machine-readable run
#   .\run_benchmarks.ps1 -Baseline b.json   # compare against a recorded run
#
# Each benchmark is run -N times.  The first run is counted (no warm-up discard)
# because the binary is already a native executable — the JIT-less interpreter
# has no warm-up effect.  Process startup cost (~8-12 ms) is included in every
# measurement; subtract bench 00_startup to get net interpreter time.
#
# Exit code: 0 = all benchmarks exited 0,  1 = at least one failed.
# ─────────────────────────────────────────────────────────────────────────────

param(
    [switch]$NoBuild,
    [int]   $N        = 5,
    [string]$Filter   = "",
    [string]$Json     = "",
    [string]$Baseline = "",
    # A run has to be this much slower than its baseline to be called a
    # regression. 25% is wide on purpose: on an idle desktop `00_startup` was
    # observed between 35 ms and 69 ms across two runs, and a threshold tighter
    # than the machine's own noise reports noise.
    [int]   $Threshold = 25
)

$ErrorActionPreference = "Stop"

$root     = $PSScriptRoot
$binary   = Join-Path $root "target\release\sz.exe"
$benchDir = Join-Path $root "benchmarks"

$env:SEREZ_HOME     = $root
$env:SEREZ_PACKAGES = Join-Path $root "tests\packages"

$W = 72  # line width for decorative borders

function Ruler([string]$ch = "═") { Write-Host ($ch * $W) }
function Pad([string]$s)          { Write-Host "  $s" }

# ── Header ───────────────────────────────────────────────────────────────────
Ruler
Pad "Serez-Code  ·  Advanced Benchmark Suite"
Pad ""
try {
    $cpu = (Get-CimInstance Win32_Processor -Property Name |
            Select-Object -First 1).Name.Trim()
    Pad "CPU  : $cpu"
} catch { Pad "CPU  : (unavailable)" }
try {
    $ramGB = [math]::Round(
        (Get-CimInstance Win32_PhysicalMemory |
         Measure-Object -Property Capacity -Sum).Sum / 1GB)
    Pad "RAM  : ${ramGB} GB"
} catch { Pad "RAM  : (unavailable)" }
Pad "OS   : $([System.Environment]::OSVersion.VersionString)"
try   { Pad "Rust : $((rustc --version 2>&1) -replace '^rustc\s+','')" }
catch { Pad "Rust : (unavailable)" }
Pad "Mode : release binary  |  Iterations per benchmark: $N"
Ruler

# ── Build ────────────────────────────────────────────────────────────────────
if (-not $NoBuild) {
    Write-Host ""
    Write-Host "  Building release binary..." -NoNewline -ForegroundColor Cyan
    $sw = [System.Diagnostics.Stopwatch]::StartNew()
    Push-Location $root
    $buildOutput = cargo build --release 2>&1
    $buildExit   = $LASTEXITCODE
    Pop-Location
    $sw.Stop()

    if ($buildExit -ne 0) {
        Write-Host " FAILED" -ForegroundColor Red
        $buildOutput | ForEach-Object { Write-Host "  $_" }
        exit 1
    }
    Write-Host (" done  ({0:F1}s)" -f $sw.Elapsed.TotalSeconds) -ForegroundColor Green
}

try {
    $ver = & $binary --version 2>&1
    Pad "Lang : $ver"
} catch {}
Ruler

# ── Discover benchmarks ───────────────────────────────────────────────────────
$allFiles = Get-ChildItem $benchDir -Filter "*.sz" -ErrorAction Stop | Sort-Object Name
if ($Filter -ne "") {
    $allFiles = $allFiles | Where-Object { $_.BaseName -like "*$Filter*" }
}
if ($allFiles.Count -eq 0) {
    Write-Host "  No benchmark files found matching filter '$Filter'." -ForegroundColor Yellow
    exit 0
}

# ── Run helper ────────────────────────────────────────────────────────────────
function Invoke-Bench([string]$path) {
    $outFile = [System.IO.Path]::GetTempFileName()
    $errFile = [System.IO.Path]::GetTempFileName()
    $sw   = [System.Diagnostics.Stopwatch]::StartNew()
    $proc = Start-Process -FilePath $binary `
                          -ArgumentList "`"$path`"" `
                          -NoNewWindow -Wait -PassThru `
                          -RedirectStandardOutput $outFile `
                          -RedirectStandardError  $errFile
    $sw.Stop()
    $exitCode = $proc.ExitCode
    $stderr   = if (Test-Path $errFile) { Get-Content $errFile -Raw } else { "" }
    Remove-Item $outFile, $errFile -ErrorAction SilentlyContinue
    return @{
        Ms       = [int]$sw.Elapsed.TotalMilliseconds
        Ok       = ($exitCode -eq 0)
        ErrLines = ($stderr ?? "").Trim()
    }
}

# ── Live progress ─────────────────────────────────────────────────────────────
Write-Host ""
Write-Host ("  Running {0} benchmarks  ×  {1} iterations..." -f $allFiles.Count, $N)
Write-Host ""

$results = @()
$idx     = 0

foreach ($f in $allFiles) {
    $idx++
    $name  = $f.BaseName
    $label = $name.PadRight(37)
    Write-Host ("  [{0,2}/{1}] {2}" -f $idx, $allFiles.Count, $label) -NoNewline

    $times  = @()
    $allOk  = $true
    $errMsg = ""

    for ($i = 0; $i -lt $N; $i++) {
        $r = Invoke-Bench $f.FullName
        $times += $r.Ms
        if (-not $r.Ok) {
            $allOk  = $false
            $errMsg = $r.ErrLines
        }
        Write-Host "." -NoNewline
    }

    $min = ($times | Measure-Object -Minimum).Minimum
    $max = ($times | Measure-Object -Maximum).Maximum
    $avg = [int](($times | Measure-Object -Average).Average)

    if ($allOk) {
        Write-Host ("  {0,5} ms avg  ({1}–{2})" -f $avg, $min, $max) -ForegroundColor Green
    } else {
        Write-Host ("  {0,5} ms avg  ({1}–{2})  ← FAILED" -f $avg, $min, $max) -ForegroundColor Red
        if ($errMsg) { Write-Host "           $errMsg" -ForegroundColor DarkRed }
    }

    $results += [PSCustomObject]@{
        Name = $name
        Min  = $min
        Avg  = $avg
        Max  = $max
        Ok   = $allOk
    }
}

# ── Startup baseline (bench 00) ───────────────────────────────────────────────
$startupAvg = 0
$baselineRow = $results | Where-Object { $_.Name -eq "00_startup" } | Select-Object -First 1
if ($baselineRow) { $startupAvg = $baselineRow.Avg }

# ── Results table ──────────────────────────────────────────────────────────────
Write-Host ""
Ruler
Pad "RESULTS  (all times in milliseconds,  process startup ≈ $($startupAvg) ms)"
Ruler
Write-Host ""

$hfmt = "  {0,-33}  {1,7}  {2,7}  {3,7}  {4,7}  {5}"
$rfmt = "  {0,-33}  {1,7}  {2,7}  {3,7}  {4,7}  {5}"

Write-Host ($hfmt -f "Benchmark", "min", "avg", "max", "net avg", "status")
Write-Host ("  " + ("─" * ($W - 2)))

$sumAvg = 0; $passed = 0; $failed = 0

foreach ($r in $results) {
    $net    = [math]::Max(0, $r.Avg - $startupAvg)
    $col    = if ($r.Ok) { "Green" } else { "Red" }
    $status = if ($r.Ok) { "PASS"  } else { "FAIL" }
    $line   = $rfmt -f $r.Name, $r.Min, $r.Avg, $r.Max, $net, $status
    Write-Host $line -ForegroundColor $col
    $sumAvg += $r.Avg
    if ($r.Ok) { $passed++ } else { $failed++ }
}

Write-Host ("  " + ("─" * ($W - 2)))
Write-Host ""
Pad ("Passed  : {0}/{1}" -f $passed, $results.Count)
Pad ("Sum avg : {0} ms  ({1:F2}s total across all benchmarks × 1 iteration)" -f $sumAvg, ($sumAvg / 1000.0))
Pad ("Net avg : sum minus startup overhead = {0} ms  ({1:F2}s interpreter work)" -f `
    ($sumAvg - $startupAvg * $results.Count), `
    (($sumAvg - $startupAvg * $results.Count) / 1000.0))
Write-Host ""
Ruler

# ── Optional comparison against a recorded run ────────────────────────────────
# The comparison uses `min`, not `avg`: a process is only ever slowed down by
# its neighbours, never sped up, so the fastest of N runs is the
# least-contaminated estimate of the work itself.
if ($Baseline -ne "") {
    if (-not (Test-Path $Baseline)) {
        Write-Host "Baseline '$Baseline' not found." -ForegroundColor Red
        exit 1
    }
    $base = Get-Content $Baseline -Raw | ConvertFrom-Json
    $byName = @{}
    foreach ($b in $base.benchmarks) { $byName[$b.name] = $b.min }

    Write-Host ""
    Pad "Comparing against $Baseline (threshold ${Threshold}%)"
    Write-Host ("  " + ("─" * ($W - 2)))
    $regressed = 0
    foreach ($r in $results) {
        if (-not $byName.ContainsKey($r.Name)) { continue }
        $b = [int]$byName[$r.Name]
        if ($b -le 0) { continue }
        $delta = [int]((($r.Min - $b) * 100) / $b)
        if ($delta -gt $Threshold) {
            Write-Host ("  {0,-33} {1,6} ms -> {2,6} ms  +{3}%" -f $r.Name, $b, $r.Min, $delta) `
                       -ForegroundColor Red
            $regressed++
        } elseif ($delta -lt (0 - $Threshold)) {
            Write-Host ("  {0,-33} {1,6} ms -> {2,6} ms  {3}%" -f $r.Name, $b, $r.Min, $delta) `
                       -ForegroundColor Green
        }
    }
    if ($regressed -eq 0) {
        Write-Host "  No benchmark exceeded the threshold." -ForegroundColor Green
    } else {
        Write-Host "  $regressed benchmark(s) slower than the threshold." -ForegroundColor Red
    }
    Write-Host ""
}

# ── Optional machine-readable run ─────────────────────────────────────────────
if ($Json -ne "") {
    $versionLine = (& $binary "--version") 2>&1 | Select-Object -First 1
    # Built by index assignment: nesting a list inside an `[ordered]@{...}`
    # literal fails with "Argument types do not match" on this PowerShell.
    $report = [ordered]@{}
    $report["schema"]     = "serez-benchmarks/1"
    $report["runner"]     = "run_benchmarks.ps1"
    $report["platform"]   = "windows"
    $report["core"]       = "$versionLine".Trim()
    $report["startedAt"]  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    $report["iterations"] = $N
    $report["statistic"]  = "min"
    $rows = New-Object System.Collections.Generic.List[object]
    foreach ($r in $results) {
        # Cast: Measure-Object returns doubles, and run_benchmarks.sh emits
        # integers. The same schema must not carry a different type per runner.
        $rows.Add([ordered]@{
            name   = $r.Name
            min    = [int]$r.Min
            avg    = [int]$r.Avg
            max    = [int]$r.Max
            status = $(if ($r.Ok) { "pass" } else { "fail" })
        })
    }
    $report["benchmarks"] = $rows.ToArray()
    $report | ConvertTo-Json -Depth 5 | Set-Content -Path $Json -Encoding UTF8
    Pad "Report written to $Json"
    Write-Host ""
}

exit $(if ($failed -gt 0) { 1 } else { 0 })
