# Build DarkDrop program using Docker
# This avoids local Anchor installation issues

Write-Host "Building DarkDrop program with Docker..." -ForegroundColor Cyan

# Check if Docker is running
$dockerRunning = docker ps 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "`nError: Docker is not running or not installed" -ForegroundColor Red
    Write-Host "`nPlease:" -ForegroundColor Yellow
    Write-Host "1. Install Docker Desktop: https://www.docker.com/products/docker-desktop" -ForegroundColor White
    Write-Host "2. Start Docker Desktop" -ForegroundColor White
    Write-Host "3. Run this script again" -ForegroundColor White
    exit 1
}

Write-Host "Docker is running" -ForegroundColor Green

# Force rebuild to pick up Dockerfile changes
Write-Host "`nBuilding Docker image (this may take a while)..." -ForegroundColor Yellow
docker build --no-cache -t darkpool-builder -f Dockerfile .

if ($LASTEXITCODE -ne 0) {
    Write-Host "`nError: Docker image build failed" -ForegroundColor Red
    exit 1
}
Write-Host "Docker image built successfully" -ForegroundColor Green

Write-Host "`nBuilding program..." -ForegroundColor Yellow

# Run build in container
docker run --rm `
    -v "${PWD}:/workspace" `
    -w /workspace `
    darkpool-builder `
    bash -c "anchor build && echo 'Build artifacts location:' && ls -lh target/deploy/darkpool.so 2>/dev/null || echo 'Binary not found in expected location'"

if ($LASTEXITCODE -eq 0) {
    Write-Host "`nBuild successful!" -ForegroundColor Green
    Write-Host "Program compiled in Docker container" -ForegroundColor Cyan
} else {
    Write-Host "`nBuild failed" -ForegroundColor Red
    Write-Host "Check errors above" -ForegroundColor Yellow
}

