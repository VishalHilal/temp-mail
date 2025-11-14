#!/bin/bash

# TempMail Setup Script
# This script sets up the TempMail server on a fresh system

set -e

echo "🚀 TempMail Setup Script"
echo "========================"
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if running as root
if [ "$EUID" -eq 0 ]; then
    echo -e "${RED}Please don't run this script as root${NC}"
    exit 1
fi

# Detect OS
if [ -f /etc/os-release ]; then
    . /etc/os-release
    OS=$ID
else
    echo -e "${RED}Cannot detect OS${NC}"
    exit 1
fi

echo "Detected OS: $OS"
echo ""

# Install Rust if not present
if ! command -v cargo &> /dev/null; then
    echo -e "${YELLOW}Installing Rust...${NC}"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    echo -e "${GREEN}✓ Rust installed${NC}"
else
    echo -e "${GREEN}✓ Rust already installed${NC}"
fi

# Install PostgreSQL
if ! command -v psql &> /dev/null; then
    echo -e "${YELLOW}Installing PostgreSQL...${NC}"
    
    if [ "$OS" = "ubuntu" ] || [ "$OS" = "debian" ]; then
        sudo apt update
        sudo apt install -y postgresql postgresql-contrib
        sudo systemctl start postgresql
        sudo systemctl enable postgresql
    elif [ "$OS" = "centos" ] || [ "$OS" = "rhel" ]; then
        sudo dnf install -y postgresql-server postgresql-contrib
        sudo postgresql-setup --initdb
        sudo systemctl start postgresql
        sudo systemctl enable postgresql
    else
        echo -e "${RED}Please install PostgreSQL manually${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}✓ PostgreSQL installed${NC}"
else
    echo -e "${GREEN}✓ PostgreSQL already installed${NC}"
fi

# Setup database
echo -e "${YELLOW}Setting up database...${NC}"
DB_PASSWORD=$(openssl rand -base64 32)

sudo -u postgres psql << EOF
CREATE DATABASE tempmail;
CREATE USER tempmail_user WITH PASSWORD '$DB_PASSWORD';
GRANT ALL PRIVILEGES ON DATABASE tempmail TO tempmail_user;
\q
EOF

echo -e "${GREEN}✓ Database created${NC}"

# Create .env file
echo -e "${YELLOW}Creating configuration...${NC}"
cat > .env << EOF
DATABASE_URL=postgres://tempmail_user:$DB_PASSWORD@localhost/tempmail
SMTP_DOMAIN=tempmail.local
RUST_LOG=info
EOF

echo -e "${GREEN}✓ Configuration created${NC}"

# Create templates directory if it doesn't exist
mkdir -p templates

echo ""
echo -e "${GREEN}Setup complete!${NC}"
echo ""
echo "Next steps:"
echo "1. Copy all HTML template files to the 'templates/' directory"
echo "2. Edit .env to set your SMTP_DOMAIN"
echo "3. Run: cargo build --release"
echo "4. Run: ./target/release/tempmail_rs"
echo ""
echo "Your database password has been saved to .env"
echo ""
echo "SMTP will listen on: 0.0.0.0:2525"
echo "HTTP will listen on: 0.0.0.0:3000"
echo ""
