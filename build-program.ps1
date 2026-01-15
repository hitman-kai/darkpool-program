# DarkDrop Program Build Script
# This script fixes lockfile version issues and builds the Anchor program

Write-Host "Building DarkDrop program..." -ForegroundColor Cyan

# Remove existing lockfiles
Remove-Item Cargo.lock -Force -ErrorAction SilentlyContinue
Remove-Item programs\darkpool\Cargo.lock -Force -ErrorAction SilentlyContinue

# Generate lockfile
Write-Host "Generating Cargo.lock..." -ForegroundColor Yellow
cd programs\darkpool
cargo generate-lockfile 2>&1 | Out-Null
cd ..\..

# Ensure lockfile is version 4 (required for newer Cargo)
if (Test-Path Cargo.lock) {
    Write-Host "Ensuring Cargo.lock is version 4..." -ForegroundColor Yellow
    $content = Get-Content Cargo.lock -Raw
    if ($content -match 'version = 3') {
        $content = $content -replace 'version = 3', 'version = 4'
        Set-Content Cargo.lock -Value $content -NoNewline
        Write-Host "Updated lockfile to version 4" -ForegroundColor Green
    } else {
        Write-Host "Lockfile is already version 4" -ForegroundColor Green
    }
}

# Build with Anchor
Write-Host "Building with Anchor..." -ForegroundColor Yellow
anchor build

if ($LASTEXITCODE -eq 0) {
    Write-Host "`nBuild successful! Program compiled." -ForegroundColor Green
    Write-Host "Program ID can be found in Anchor.toml or target/idl/darkpool.json" -ForegroundColor Cyan
} else {
    Write-Host "`nBuild failed. Check errors above." -ForegroundColor Red
    Write-Host "`nNote: If you see Rust version errors, Anchor 0.32.1 requires Rust 1.76+," -ForegroundColor Yellow
    Write-Host "but the Solana toolchain uses Rust 1.75. Consider using Anchor 0.30.0 instead." -ForegroundColor Yellow
}

