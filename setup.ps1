# TempMail Windows Setup Script
# Run this in PowerShell as Administrator

Write-Host "🚀 TempMail Windows Setup Script" -ForegroundColor Cyan
Write-Host "=================================" -ForegroundColor Cyan
Write-Host ""

# Check if running as Administrator
$currentPrincipal = New-Object Security.Principal.WindowsPrincipal([Security.Principal.WindowsIdentity]::GetCurrent())
$isAdmin = $currentPrincipal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

if (-not $isAdmin) {
    Write-Host "❌ Please run this script as Administrator" -ForegroundColor Red
    exit 1
}

# Install Chocolatey if not present
if (-not (Get-Command choco -ErrorAction SilentlyContinue)) {
    Write-Host "📦 Installing Chocolatey..." -ForegroundColor Yellow
    Set-ExecutionPolicy Bypass -Scope Process -Force
    [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.ServicePointManager]::SecurityProtocol -bor 3072
    Invoke-Expression ((New-Object System.Net.WebClient).DownloadString('https://community.chocolatey.org/install.ps1'))
    Write-Host "✓ Chocolatey installed" -ForegroundColor Green
} else {
    Write-Host "✓ Chocolatey already installed" -ForegroundColor Green
}

# Install Rust
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "🦀 Installing Rust..." -ForegroundColor Yellow
    choco install rust -y
    refreshenv
    Write-Host "✓ Rust installed" -ForegroundColor Green
} else {
    Write-Host "✓ Rust already installed" -ForegroundColor Green
}

# Install PostgreSQL
if (-not (Get-Command psql -ErrorAction SilentlyContinue)) {
    Write-Host "🐘 Installing PostgreSQL..." -ForegroundColor Yellow
    choco install postgresql15 -y --params '/Password:postgres'
    refreshenv
    
    # Wait for PostgreSQL to start
    Start-Sleep -Seconds 10
    Write-Host "✓ PostgreSQL installed" -ForegroundColor Green
} else {
    Write-Host "✓ PostgreSQL already installed" -ForegroundColor Green
}

# Setup database
Write-Host "🗄️  Setting up database..." -ForegroundColor Yellow

# Generate random password
$DB_PASSWORD = -join ((65..90) + (97..122) + (48..57) | Get-Random -Count 32 | % {[char]$_})

# Create SQL script
$sqlScript = @"
CREATE DATABASE tempmail;
CREATE USER tempmail_user WITH PASSWORD '$DB_PASSWORD';
GRANT ALL PRIVILEGES ON DATABASE tempmail TO tempmail_user;
"@

$sqlScript | & "C:\Program Files\PostgreSQL\15\bin\psql.exe" -U postgres

Write-Host "✓ Database created" -ForegroundColor Green

# Create .env file
Write-Host "⚙️  Creating configuration..." -ForegroundColor Yellow

$envContent = @"
DATABASE_URL=postgres://tempmail_user:$DB_PASSWORD@localhost/tempmail
SMTP_DOMAIN=tempmail.local
RUST_LOG=info
"@

$envContent | Out-File -FilePath ".env" -Encoding UTF8

Write-Host "✓ Configuration created" -ForegroundColor Green

# Create templates directory
if (-not (Test-Path "templates")) {
    New-Item -ItemType Directory -Path "templates" | Out-Null
}

Write-Host ""
Write-Host "✅ Setup complete!" -ForegroundColor Green
Write-Host ""
Write-Host "Next steps:"
Write-Host "1. Copy all HTML template files to the 'templates\' directory"
Write-Host "2. Edit .env to set your SMTP_DOMAIN"
Write-Host "3. Run: cargo build --release"
Write-Host "4. Run: .\target\release\tempmail_rs.exe"
Write-Host ""
Write-Host "Your database password has been saved to .env"
Write-Host ""
Write-Host "SMTP will listen on: 0.0.0.0:2525"
Write-Host "HTTP will listen on: 0.0.0.0:3000"
Write-Host ""
Write-Host "Press any key to continue..."
$null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
