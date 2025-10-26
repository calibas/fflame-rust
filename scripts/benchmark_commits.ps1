# Benchmark script for comparing performance across commits
# Runs multiple iterations to account for variance

param(
    [string]$ConfigFile = "tests/visual/configs/complex.fflame",
    [int]$Iterations = 5,
    [int]$Width = 1920,
    [int]$Height = 1080
)

$commits = @(
    @{Hash="dd80003"; Name="Before Histogram (textureStore)"},
    @{Hash="ef0cdd8"; Name="Histogram Fixed (naive atomic)"},
    @{Hash="06bfcab"; Name="Histogram + Local Cache (current)"}
)

$resultsDir = "benchmark_results"
New-Item -ItemType Directory -Force -Path $resultsDir | Out-Null

# Extract fractal name from config file path
$fractalName = [System.IO.Path]::GetFileNameWithoutExtension($ConfigFile)

# Get test timestamp
$timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"

# Get system info
$platform = $env:OS
if ([string]::IsNullOrEmpty($platform)) {
    $platform = (uname -s 2>$null) -replace "`n", ""
    if ([string]::IsNullOrEmpty($platform)) { $platform = "Unknown" }
}

Write-Host "======================================" -ForegroundColor Cyan
Write-Host "Fractal Flame Performance Benchmark" -ForegroundColor Cyan
Write-Host "======================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Fractal: $fractalName"
Write-Host "Config: $ConfigFile"
Write-Host "Timestamp: $timestamp"
Write-Host "Platform: $platform"
Write-Host "Iterations per commit: $Iterations"
Write-Host "Resolution: ${Width}x${Height}"
Write-Host ""

# Store current branch
$originalBranch = git rev-parse --abbrev-ref HEAD

# Initialize or load CSV with comprehensive headers
$csvPath = "$resultsDir/benchmark_history.csv"
if (-not (Test-Path $csvPath)) {
    $headers = "Timestamp,Fractal,ConfigFile,Resolution,Platform,Commit,CommitName," +
               "TotalIterations,IterationsPerThread,SpeedFactor,Workgroups," +
               "Mean_ms,StdDev_ms,CV_percent,Min_ms,Max_ms," +
               "Throughput_Giter_sec,Samples,RustcVersion,BuildProfile"
    $headers | Out-File -FilePath $csvPath -Encoding UTF8
}

foreach ($commit in $commits) {
    $hash = $commit.Hash
    $name = $commit.Name

    Write-Host "======================================" -ForegroundColor Yellow
    Write-Host "Testing: $name" -ForegroundColor Yellow
    Write-Host "Commit: $hash" -ForegroundColor Yellow
    Write-Host "======================================" -ForegroundColor Yellow

    # Checkout commit
    git checkout $hash 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Failed to checkout $hash" -ForegroundColor Red
        continue
    }

    # Build in release mode
    Write-Host "Building..." -ForegroundColor Cyan
    cargo build --release 2>&1 | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Build failed for $hash" -ForegroundColor Red
        continue
    }

    # Run benchmark iterations and collect metadata
    $times = @()
    $metadataCache = $null
    for ($i = 1; $i -le $Iterations; $i++) {
        Write-Host "  Run $i/$Iterations..." -NoNewline

        $outputFile = "$resultsDir/${hash}_run${i}.png"

        # Run export and capture output
        $output = & cargo run --release -- export -i $ConfigFile -o $outputFile --width $Width --height $Height 2>&1

        if ($LASTEXITCODE -eq 0) {
            # Extract full metadata from PNG
            $metadataOutput = & cargo run --release --bin compare_images -- -1 $outputFile -2 $outputFile --skip-color-check 2>&1

            # Extract render time
            if ($metadataOutput -match "Render Time: ([\d.]+)ms") {
                $time = [double]$matches[1]
                $times += $time
                Write-Host " ${time}ms" -ForegroundColor Green

                # Cache metadata from first run
                if ($null -eq $metadataCache) {
                    $metadataCache = @{}
                    if ($metadataOutput -match "Total Iterations: ([\d]+)") { $metadataCache.TotalIterations = $matches[1] }
                    if ($metadataOutput -match "Iterations/Thread: ([\d]+)") { $metadataCache.IterPerThread = $matches[1] }
                    if ($metadataOutput -match "Speed Factor: ([\d.]+)") { $metadataCache.SpeedFactor = $matches[1] }
                    if ($metadataOutput -match "Rustc Version: ([\d.]+)") { $metadataCache.RustcVersion = $matches[1] }
                    if ($metadataOutput -match "Build Profile: (\w+)") { $metadataCache.BuildProfile = $matches[1] }
                }
            } else {
                Write-Host " (failed to extract time)" -ForegroundColor Red
            }
        } else {
            Write-Host " (export failed)" -ForegroundColor Red
        }
    }

    # Calculate statistics
    if ($times.Count -gt 0) {
        $mean = ($times | Measure-Object -Average).Average
        $min = ($times | Measure-Object -Minimum).Minimum
        $max = ($times | Measure-Object -Maximum).Maximum

        # Calculate standard deviation and coefficient of variation
        $variance = ($times | ForEach-Object { [Math]::Pow($_ - $mean, 2) } | Measure-Object -Average).Average
        $stddev = [Math]::Sqrt($variance)
        $cv = ($stddev / $mean) * 100

        # Calculate throughput (Giter/sec)
        $totalIter = [double]$metadataCache.TotalIterations
        $throughput = ($totalIter / ($mean / 1000.0)) / 1000000000.0

        # Calculate workgroups (assuming 128 workgroups default)
        $workgroups = 128

        Write-Host ""
        Write-Host "Statistics:" -ForegroundColor Cyan
        Write-Host "  Mean:       ${mean:N2} ms" -ForegroundColor White
        Write-Host "  StdDev:     ${stddev:N2} ms (${cv:N2}%)" -ForegroundColor White
        Write-Host "  Min:        ${min:N2} ms" -ForegroundColor White
        Write-Host "  Max:        ${max:N2} ms" -ForegroundColor White
        Write-Host "  Throughput: ${throughput:N2} Giter/sec" -ForegroundColor White
        Write-Host ""

        # Save comprehensive statistics to CSV
        $resolution = "${Width}x${Height}"
        $row = "$timestamp,$fractalName,`"$ConfigFile`",$resolution,$platform,$hash,`"$name`"," +
               "$($metadataCache.TotalIterations),$($metadataCache.IterPerThread),$($metadataCache.SpeedFactor),$workgroups," +
               "$mean,$stddev,$cv,$min,$max," +
               "$throughput,$($times.Count),$($metadataCache.RustcVersion),$($metadataCache.BuildProfile)"
        $row | Out-File -FilePath $csvPath -Append -Encoding UTF8
    }
}

# Return to original branch
Write-Host "======================================" -ForegroundColor Cyan
Write-Host "Returning to original branch..." -ForegroundColor Cyan
git checkout $originalBranch 2>&1 | Out-Null

Write-Host ""
Write-Host "Benchmark complete!" -ForegroundColor Green
Write-Host "Results saved to: $resultsDir" -ForegroundColor Green
Write-Host ""
Write-Host "Benchmark History: $resultsDir/benchmark_history.csv" -ForegroundColor Cyan
Write-Host ""
Write-Host "All benchmark data is appended to benchmark_history.csv for long-term tracking." -ForegroundColor Gray
