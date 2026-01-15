# Deploy DarkDrop program to devnet
# This script uses Docker to deploy the program
# Set KEYPAIR_PATH environment variable or update the default path below

Write-Host "Deploying DarkDrop program to devnet..." -ForegroundColor Cyan

# Check if Docker is running
$dockerRunning = docker ps 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "`nError: Docker is not running or not installed" -ForegroundColor Red
    exit 1
}

Write-Host "Docker is running" -ForegroundColor Green

# Check if program is built
if (-not (Test-Path "target/deploy/darkpool.so")) {
    Write-Host "`nWarning: Program not built yet. Building first..." -ForegroundColor Yellow
    .\build-with-docker.ps1
    if ($LASTEXITCODE -ne 0) {
        Write-Host "`nError: Build failed. Cannot deploy." -ForegroundColor Red
        exit 1
    }
}

Write-Host "`nProgram binary found" -ForegroundColor Green

# Find keypair file - use environment variable or default
$keypairPath = $env:KEYPAIR_PATH
if (-not $keypairPath) {
    # Default to Solana CLI default location
    $solanaConfig = solana config get 2>&1 | Select-String "Keypair Path"
    if ($solanaConfig) {
        $keypairPath = ($solanaConfig -split ":")[1].Trim()
    } else {
        Write-Host "`nError: Keypair path not found" -ForegroundColor Red
        Write-Host "Set KEYPAIR_PATH environment variable or configure Solana CLI" -ForegroundColor Yellow
        Write-Host "Example: `$env:KEYPAIR_PATH = 'path/to/keypair.json'" -ForegroundColor Gray
        exit 1
    }
}

if (-not (Test-Path $keypairPath)) {
    Write-Host "`nError: Keypair file not found: $keypairPath" -ForegroundColor Red
    Write-Host "Set KEYPAIR_PATH environment variable or update Solana config" -ForegroundColor Yellow
    exit 1
}

Write-Host "`nUsing keypair: $keypairPath" -ForegroundColor Cyan

# Deploy using Docker
Write-Host "`nDeploying to devnet..." -ForegroundColor Yellow
Write-Host "Note: Make sure you have devnet SOL in your wallet!" -ForegroundColor Cyan
Write-Host "Get free SOL: solana airdrop 2 --url devnet" -ForegroundColor Gray

# Get the directory and filename of the keypair
$keypairDir = Split-Path -Parent $keypairPath
$keypairFile = Split-Path -Leaf $keypairPath

docker run --rm `
    -v "${PWD}:/workspace" `
    -v "${keypairDir}:/keys" `
    -w /workspace `
    darkpool-builder `
    bash -c "solana config set --url devnet && solana config set --keypair /keys/$keypairFile && anchor deploy --provider.cluster devnet --provider.wallet /keys/$keypairFile"

if ($LASTEXITCODE -eq 0) {
    Write-Host "`nDeployment successful!" -ForegroundColor Green
    Write-Host "`nNext steps:" -ForegroundColor Cyan
    Write-Host "1. Update program ID in your code if it changed" -ForegroundColor White
    Write-Host "2. Test the program with your app" -ForegroundColor White
    Write-Host "3. Check deployment: solana program show <PROGRAM_ID> --url devnet" -ForegroundColor White
} else {
    Write-Host "`nDeployment failed" -ForegroundColor Red
    Write-Host "Check errors above" -ForegroundColor Yellow
    Write-Host "`nCommon issues:" -ForegroundColor Yellow
    Write-Host "- Not enough SOL: solana airdrop 2 --url devnet" -ForegroundColor White
    Write-Host "- Wrong network: solana config set --url devnet" -ForegroundColor White
    Write-Host "- Wallet not found: Check Solana config keypair path" -ForegroundColor White
}


