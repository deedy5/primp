# PRIMP Test Runner Build Script
param(
    [ValidateSet("Debug", "Release")]
    [string]$Configuration = "Release",
    
    [ValidateSet("x64", "x86")]
    [string]$Platform = "x64",
    
    [switch]$Clean,
    [switch]$Verbose
)

$ErrorActionPreference = "Stop"

Write-Host "PRIMP Test Runner Build Script" -ForegroundColor Green
Write-Host "Configuration: $Configuration" -ForegroundColor Cyan
Write-Host "Platform: $Platform" -ForegroundColor Cyan

# Set build directory - go up one level from build/ to project root
$ProjectRoot = Split-Path -Parent $PSScriptRoot
$VenvPath = Join-Path $ProjectRoot "venv"
$PythonExe = Join-Path $VenvPath "Scripts\python.exe"
$MaturinExe = Join-Path $VenvPath "Scripts\maturin.exe"

Write-Host "Project Root: $ProjectRoot" -ForegroundColor Gray
Write-Host "Virtual Environment: $VenvPath" -ForegroundColor Gray

# Verify we're in the right directory
$CargoToml = Join-Path $ProjectRoot "Cargo.toml"
if (-not (Test-Path $CargoToml)) {
    Write-Host "Error: Cargo.toml not found at $CargoToml" -ForegroundColor Red
    Write-Host "Please run this script from the build/ directory of a valid PRIMP project" -ForegroundColor Red
    exit 1
}

Write-Host "Found Cargo.toml at: $CargoToml" -ForegroundColor Green

# Function to check if a command exists
function Test-Command {
    param([string]$Command)
    $null = Get-Command $Command -ErrorAction SilentlyContinue
    return $?
}

# Function to install Rust
function Install-Rust {
    Write-Host "Installing Rust..." -ForegroundColor Yellow
    
    try {
        # Download rustup-init
        $rustupInit = "$env:TEMP\rustup-init.exe"
        Write-Host "Downloading Rust installer..." -ForegroundColor Gray
        
        try {
            Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile $rustupInit -UseBasicParsing
        }
        catch {
            Write-Host "Trying alternative download method..." -ForegroundColor Yellow
            $webClient = New-Object System.Net.WebClient
            $webClient.DownloadFile("https://win.rustup.rs/x86_64", $rustupInit)
        }
        
        if (-not (Test-Path $rustupInit)) {
            throw "Failed to download Rust installer"
        }
        
        # Install Rust with default settings
        Write-Host "Installing Rust (this may take a few minutes)..." -ForegroundColor Gray
        $process = Start-Process -FilePath $rustupInit -ArgumentList "-y", "--default-toolchain", "stable" -Wait -PassThru -NoNewWindow
        
        if ($process.ExitCode -ne 0) {
            throw "Rust installation failed with exit code $($process.ExitCode)"
        }
        
        # Clean up installer
        Remove-Item $rustupInit -Force -ErrorAction SilentlyContinue
        
        # Add Rust to PATH for this session
        $cargoPath = "$env:USERPROFILE\.cargo\bin"
        if (Test-Path $cargoPath) {
            $env:Path = "$cargoPath;$env:Path"
            Write-Host "Rust installed successfully!" -ForegroundColor Green
        }
        else {
            throw "Rust installation completed but cargo not found at expected location"
        }
    }
    catch {
        Write-Host "Failed to install Rust: $_" -ForegroundColor Red
        throw "Rust installation failed"
    }
}

# Function to setup Rust environment
function Setup-RustEnvironment {
    Write-Host "Setting up Rust environment..." -ForegroundColor Yellow
    
    $cargoPath = "$env:USERPROFILE\.cargo\bin"
    if (Test-Path "$cargoPath\cargo.exe") {
        if ($env:Path -notlike "*$cargoPath*") {
            $env:Path = "$cargoPath;$env:Path"
            Write-Host "Added Rust to PATH: $cargoPath" -ForegroundColor Gray
        }
    }
    
    if (-not (Test-Command "rustc")) {
        if (Test-Path "$cargoPath\cargo.exe") {
            $env:Path = "$cargoPath;$env:Path"
            Write-Host "Found Rust at: $cargoPath" -ForegroundColor Green
        }
        else {
            Write-Host "Rust not found. Installing automatically..." -ForegroundColor Yellow
            Install-Rust
        }
    }
    else {
        Write-Host "Rust already installed: $(rustc --version)" -ForegroundColor Green
    }
    
    if (-not (Test-Command "cargo")) {
        if (Test-Path "$cargoPath\cargo.exe") {
            $env:Path = "$cargoPath;$env:Path"
        }
        else {
            throw "Cargo not found even after Rust installation"
        }
    }
    
    try {
        $rustVersion = & rustc --version
        $cargoVersion = & cargo --version
        Write-Host "Rust ready: $rustVersion" -ForegroundColor Green
        Write-Host "Cargo ready: $cargoVersion" -ForegroundColor Green
    }
    catch {
        throw "Failed to verify Rust installation: $_"
    }
}

# Setup Rust environment
Setup-RustEnvironment

# Environment setup for PRIMP
$env:RUSTFLAGS = "-C target-cpu=native"
$env:RUST_BACKTRACE = if ($Verbose) { "full" } else { "1" }

Write-Host "Environment configured for PRIMP build:" -ForegroundColor Cyan
Write-Host "  RUSTFLAGS: $env:RUSTFLAGS" -ForegroundColor White

# Clean if requested
if ($Clean) {
    Write-Host "Cleaning build artifacts..." -ForegroundColor Yellow
    
    Push-Location $ProjectRoot
    try {
        if (Test-Path "target") { 
            Write-Host "Removing target directory..." -ForegroundColor Gray
            Remove-Item -Recurse -Force "target" 
        }
        if (Test-Path $VenvPath) { 
            Write-Host "Removing virtual environment..." -ForegroundColor Gray
            Remove-Item -Recurse -Force $VenvPath 
        }
        if (Test-Path "Cargo.lock") { 
            Write-Host "Removing Cargo.lock..." -ForegroundColor Gray
            Remove-Item -Force "Cargo.lock" 
        }
        
        Write-Host "Clean completed!" -ForegroundColor Green
    }
    finally {
        Pop-Location
    }
    
    if ($Clean.IsPresent) {
        return
    }
}

# Setup Python environment
Write-Host "Setting up Python environment..." -ForegroundColor Yellow

Push-Location $ProjectRoot
try {
    if (-not (Test-Path $VenvPath)) {
        Write-Host "Creating Python virtual environment..." -ForegroundColor Gray
        python -m venv venv
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to create Python virtual environment"
        }
    }

    # Install core dependencies including certifi and other test requirements
    Write-Host "Installing Python dependencies..." -ForegroundColor Gray
    & $PythonExe -m pip install --upgrade pip setuptools wheel maturin pytest pytest-asyncio pytest-rerunfailures certifi requests urllib3 --quiet
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to install Python dependencies"
    }
    
    Write-Host "Python environment ready!" -ForegroundColor Green
}
finally {
    Pop-Location
}

# Clean cargo build
Write-Host "Cleaning Cargo cache..." -ForegroundColor Gray
Push-Location $ProjectRoot
try {
    & cargo clean
    if (Test-Path "Cargo.lock") { 
        Remove-Item -Force "Cargo.lock" 
    }
    Write-Host "Cargo cache cleared!" -ForegroundColor Green
}
finally {
    Pop-Location
}

# Build with maturin
Write-Host "Building Rust extension with maturin..." -ForegroundColor Yellow

$maturinArgs = @("build", "--strip")
if ($Configuration -eq "Release") {
    $maturinArgs += "--release"
}

if ($Verbose) {
    $maturinArgs += "--verbose"
}
else {
    $maturinArgs += "--quiet"
}

Write-Host "Executing: maturin $($maturinArgs -join ' ')" -ForegroundColor Gray

Push-Location $ProjectRoot
try {
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    
    Write-Host "Starting maturin build..." -ForegroundColor Gray
    & $MaturinExe @maturinArgs
    if ($LASTEXITCODE -ne 0) {
        Write-Host "Maturin build failed with exit code: $LASTEXITCODE" -ForegroundColor Red
        throw "Maturin build failed"
    }
    
    Write-Host "Maturin build completed successfully!" -ForegroundColor Green
}
finally {
    Pop-Location
}

# Install the built wheel
Write-Host "Installing PRIMP wheel..." -ForegroundColor Yellow

Push-Location $ProjectRoot
try {
    $wheelFiles = Get-ChildItem -Path "target\wheels\*.whl" -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending
    if ($wheelFiles.Count -eq 0) {
        throw "No wheel file found in target\wheels\"
    }

    $latestWheel = $wheelFiles[0]
    Write-Host "Installing: $($latestWheel.Name)" -ForegroundColor Gray

    & $PythonExe -m pip install $latestWheel.FullName --force-reinstall --quiet
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to install PRIMP wheel"
    }
    
    Write-Host "PRIMP wheel installed successfully!" -ForegroundColor Green
}
finally {
    Pop-Location
}

# Verify installation
Write-Host "Verifying PRIMP installation..." -ForegroundColor Yellow
Push-Location $ProjectRoot
try {
    & $PythonExe -c "import primp; print('PRIMP imported successfully')"
    if ($LASTEXITCODE -eq 0) {
        Write-Host "PRIMP verification passed!" -ForegroundColor Green
    }
    else {
        Write-Host "Warning: PRIMP import test failed" -ForegroundColor Yellow
    }
}
catch {
    Write-Host "Warning: Could not verify PRIMP installation: $_" -ForegroundColor Yellow
}
finally {
    Pop-Location
}

# Install test dependencies from file if it exists, otherwise skip
Push-Location $ProjectRoot
try {
    $testReqFile = "test-requirements.txt"
    if (Test-Path $testReqFile) {
        Write-Host "Installing test dependencies from $testReqFile..." -ForegroundColor Yellow
        & $PythonExe -m pip install -r $testReqFile --quiet
        if ($LASTEXITCODE -ne 0) {
            Write-Host "Warning: Failed to install some test dependencies" -ForegroundColor Yellow
        }
        else {
            Write-Host "Test dependencies installed successfully!" -ForegroundColor Green
        }
    }
    else {
        Write-Host "No test-requirements.txt found, using basic dependencies" -ForegroundColor Gray
    }
}
catch {
    Write-Host "Warning: Error installing test dependencies: $_" -ForegroundColor Yellow
}
finally {
    Pop-Location
}

# Run the tests
Write-Host "Running PRIMP tests..." -ForegroundColor Yellow

Push-Location $ProjectRoot
try {
    $testsDir = "tests"
    if (-not (Test-Path $testsDir)) {
        Write-Host "Tests directory not found: $testsDir" -ForegroundColor Red
        Write-Host "Please create the tests directory and place your test files there" -ForegroundColor Yellow
        throw "Tests directory not found"
    }
    
    $testFiles = @(
        "test_asyncclient.py",
        "test_client.py", 
        "test_defs.py",
        "test_response.py"
    )
    
    $testsRun = 0
    $testsFailed = 0
    
    foreach ($testFile in $testFiles) {
        $testPath = Join-Path $testsDir $testFile
        if (Test-Path $testPath) {
            Write-Host "Running $testFile..." -ForegroundColor Gray
            & $PythonExe -m pytest $testPath -v --tb=short
            if ($LASTEXITCODE -eq 0) {
                Write-Host "$testFile PASSED" -ForegroundColor Green
            }
            else {
                Write-Host "$testFile FAILED" -ForegroundColor Red
                $testsFailed++
            }
            $testsRun++
        }
        else {
            Write-Host "Test file not found: $testPath" -ForegroundColor Yellow
        }
    }
    
    Write-Host ""
    Write-Host "Test Summary:" -ForegroundColor Cyan
    Write-Host "  Tests run: $testsRun" -ForegroundColor White
    Write-Host "  Tests failed: $testsFailed" -ForegroundColor $(if ($testsFailed -eq 0) { 'Green' } else { 'Red' })
    Write-Host "  Tests passed: $($testsRun - $testsFailed)" -ForegroundColor Green
    
    if ($testsFailed -eq 0 -and $testsRun -gt 0) {
        Write-Host "All tests passed!" -ForegroundColor Green
    }
    elseif ($testsFailed -gt 0) {
        Write-Host "Some tests failed!" -ForegroundColor Red
    }
    else {
        Write-Host "No tests were found to run!" -ForegroundColor Yellow
    }
}
finally {
    Pop-Location
}

Write-Host ""
Write-Host "PRIMP build and test process completed!" -ForegroundColor Green