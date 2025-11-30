# Deploy DarkDrop program to devnet
# This script uses Docker to deploy the program

Write-Host "Deploying DarkDrop program to devnet..." -ForegroundColor Cyan

# Check if Docker is running
$dockerRunning = docker ps 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "`n❌ Docker is not running or not installed" -ForegroundColor Red
    exit 1
}

Write-Host "✅ Docker is running" -ForegroundColor Green

# Check if program is built
if (-not (Test-Path "target/deploy/darkdrop.so")) {
    Write-Host "`n⚠️  Program not built yet. Building first..." -ForegroundColor Yellow
    .\build-with-docker.ps1
    if ($LASTEXITCODE -ne 0) {
        Write-Host "`n❌ Build failed. Cannot deploy." -ForegroundColor Red
        exit 1
    }
}

Write-Host "`n✅ Program binary found" -ForegroundColor Green

# Find keypair file
$keypairPath = "D:\Dev\Keys\darkdrop-funding.json"
if (-not (Test-Path $keypairPath)) {
    Write-Host "`n❌ Keypair file not found: $keypairPath" -ForegroundColor Red
    Write-Host "Please update the keypair path in deploy-devnet.ps1" -ForegroundColor Yellow
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
    darkdrop-builder `
    bash -c "solana config set --url devnet && solana config set --keypair /keys/$keypairFile && anchor deploy --provider.cluster devnet --provider.wallet /keys/$keypairFile"

if ($LASTEXITCODE -eq 0) {
    Write-Host "`n✅ Deployment successful!" -ForegroundColor Green
    Write-Host "`nNext steps:" -ForegroundColor Cyan
    Write-Host "1. Update program ID in your code if it changed" -ForegroundColor White
    Write-Host "2. Test the program with your app" -ForegroundColor White
    Write-Host "3. Check deployment: solana program show <PROGRAM_ID> --url devnet" -ForegroundColor White
} else {
    Write-Host "`n❌ Deployment failed" -ForegroundColor Red
    Write-Host "Check errors above" -ForegroundColor Yellow
    Write-Host "`nCommon issues:" -ForegroundColor Yellow
    Write-Host "- Not enough SOL: solana airdrop 2 --url devnet" -ForegroundColor White
    Write-Host "- Wrong network: solana config set --url devnet" -ForegroundColor White
    Write-Host "- Wallet not found: Check ~/.config/solana/id.json" -ForegroundColor White
}


