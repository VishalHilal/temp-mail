# TempMail RS - Production-Ready Temporary Email Server

A complete, modern temporary email service built in Rust with SMTP server and beautiful web interface.

## Features

### Core Features
- ✉️ **Full SMTP Server** - Receives emails on port 2525
- 🌐 **Modern Web Interface** - Beautiful, responsive UI
- 🎲 **Random or Custom Emails** - Generate random addresses or create custom ones
- ⚡ **Real-time Updates** - Auto-refresh inbox every 10 seconds
- 📧 **Email Parsing** - Supports text, HTML, and raw email viewing
- 🗄️ **PostgreSQL Storage** - Reliable database backend
- 🔒 **No Registration** - Completely anonymous
- 🗑️ **Auto-Cleanup** - Built-in cleanup for old emails

### SMTP Features
- HELO/EHLO support
- MAIL FROM, RCPT TO commands
- DATA command with proper email reception
- Multiple recipient support
- Email validation
- Automatic mailbox creation

### Web Features
- Copy email to clipboard
- View inbox with message list
- Read individual messages
- Switch between text/HTML/raw views
- Download raw email files
- Print emails
- Mobile-responsive design

## Prerequisites

- Rust 1.70+ (install from [rustup.rs](https://rustup.rs))
- PostgreSQL 12+
- Git

## Quick Start

### 1. Install PostgreSQL

**Ubuntu/Debian:**
```bash
sudo apt update
sudo apt install postgresql postgresql-contrib
sudo systemctl start postgresql
```

**macOS:**
```bash
brew install postgresql@15
brew services start postgresql@15
```

**Windows:**
Download from [postgresql.org](https://www.postgresql.org/download/windows/)

### 2. Create Database

```bash
sudo -u postgres psql
CREATE DATABASE tempmail;
CREATE USER tempmail_user WITH PASSWORD 'your_secure_password';
GRANT ALL PRIVILEGES ON DATABASE tempmail TO tempmail_user;
\q
```

### 3. Clone and Setup

```bash
git clone <your-repo-url>
cd temp_mail

# Copy environment file
cp .env.example .env

# Edit .env with your database credentials
nano .env
```

### 4. Configure Environment

Edit `.env`:
```bash
DATABASE_URL=postgres://tempmail_user:your_secure_password@localhost/tempmail
SMTP_DOMAIN=yourdomain.com
RUST_LOG=info
```

### 5. Create Templates Directory

```bash
mkdir -p templates
# Copy all .html template files to this directory
```

### 6. Build and Run

```bash
# Development
cargo run

# Production (optimized)
cargo build --release
./target/release/tempmail_rs
```

## Testing the SMTP Server

### Using telnet (Linux/Mac)

```bash
telnet localhost 2525
```

Then type:
```
EHLO localhost
MAIL FROM:<sender@example.com>
RCPT TO:<test@yourdomain.com>
DATA
Subject: Test Email
From: sender@example.com
To: test@yourdomain.com

This is a test email body.
.
QUIT
```

### Using PowerShell (Windows)

```powershell
$client = New-Object System.Net.Sockets.TcpClient("localhost", 2525)
$stream = $client.GetStream()
$writer = New-Object System.IO.StreamWriter($stream)
$reader = New-Object System.IO.StreamReader($stream)

$reader.ReadLine()  # Read greeting
$writer.WriteLine("EHLO localhost"); $writer.Flush()
$reader.ReadLine()
$writer.WriteLine("MAIL FROM:<test@example.com>"); $writer.Flush()
$reader.ReadLine()
$writer.WriteLine("RCPT TO:<test@yourdomain.com>"); $writer.Flush()
$reader.ReadLine()
$writer.WriteLine("DATA"); $writer.Flush()
$reader.ReadLine()
$writer.WriteLine("Subject: Test`r`nFrom: test@example.com`r`n`r`nTest body`r`n."); $writer.Flush()
$reader.ReadLine()
$writer.WriteLine("QUIT"); $writer.Flush()
$client.Close()
```

### Using Python

```python
import smtplib
from email.mime.text import MIMEText

msg = MIMEText("Test email body")
msg['Subject'] = 'Test Email'
msg['From'] = 'sender@example.com'
msg['To'] = 'test@yourdomain.com'

with smtplib.SMTP('localhost', 2525) as server:
    server.send_message(msg)
```

## Production Deployment

### 1. DNS Configuration

Set up MX records for your domain:
```
MX 10 mail.yourdomain.com
A mail.yourdomain.com 1.2.3.4
```

### 2. Firewall Rules

```bash
# Allow SMTP
sudo ufw allow 2525/tcp

# Allow HTTP
sudo ufw allow 3000/tcp

# Or use standard ports with reverse proxy
sudo ufw allow 25/tcp
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
```

### 3. Systemd Service

Create `/etc/systemd/system/tempmail.service`:

```ini
[Unit]
Description=TempMail SMTP Server
After=network.target postgresql.service

[Service]
Type=simple
User=tempmail
WorkingDirectory=/opt/tempmail
Environment="DATABASE_URL=postgres://user:pass@localhost/tempmail"
Environment="SMTP_DOMAIN=yourdomain.com"
Environment="RUST_LOG=info"
ExecStart=/opt/tempmail/target/release/tempmail_rs
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl daemon-reload
sudo systemctl enable tempmail
sudo systemctl start tempmail
sudo systemctl status tempmail
```

### 4. Nginx Reverse Proxy

```nginx
server {
    listen 80;
    server_name yourdomain.com;

    location / {
        proxy_pass http://localhost:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

### 5. SSL with Let's Encrypt

```bash
sudo apt install certbot python3-certbot-nginx
sudo certbot --nginx -d yourdomain.com
```

## Maintenance

### Cleanup Old Data

Add to crontab:
```bash
# Run daily at 2 AM
0 2 * * * psql $DATABASE_URL -c "DELETE FROM messages WHERE received_at < NOW() - INTERVAL '7 days'"
0 3 * * * psql $DATABASE_URL -c "DELETE FROM mailboxes WHERE created_at < NOW() - INTERVAL '30 days'"
```

### Monitoring Logs

```bash
# Follow logs
journalctl -u tempmail -f

# View recent logs
journalctl -u tempmail -n 100
```

### Database Backup

```bash
pg_dump tempmail > backup_$(date +%Y%m%d).sql
```

## API Endpoints

- `GET /` - Home page
- `POST /create` - Create mailbox (form: custom=string or empty)
- `GET /inbox/:local` - View inbox for email
- `GET /inbox/:local/:id` - View specific message
- `GET /api/check/:local` - JSON API to check for new messages

## Configuration Options

| Variable | Default | Description |
|----------|---------|-------------|
| DATABASE_URL | postgres://postgres:postgres@localhost/tempmail | PostgreSQL connection string |
| SMTP_DOMAIN | tempmail.local | Domain for email addresses |
| RUST_LOG | info | Log level (error, warn, info, debug, trace) |

## Troubleshooting

### Port Already in Use
```bash
# Check what's using the port
sudo lsof -i :2525
sudo lsof -i :3000

# Kill the process or change ports in code
```

### Database Connection Failed
```bash
# Test database connection
psql $DATABASE_URL

# Check PostgreSQL is running
sudo systemctl status postgresql
```

### Emails Not Appearing
```bash
# Check SMTP logs
journalctl -u tempmail | grep SMTP

# Test SMTP directly with telnet
telnet localhost 2525
```

## Security Considerations

1. **Rate Limiting** - Implement rate limiting in production
2. **Spam Prevention** - Add spam filtering if needed
3. **Size Limits** - Email size is limited to 10MB
4. **Input Validation** - All inputs are validated
5. **SQL Injection** - Using SQLx prevents SQL injection
6. **XSS Protection** - HTML emails are sandboxed in iframes

## Performance

- **Async/Await** - Fully asynchronous for high concurrency
- **Connection Pooling** - Database connection pooling
- **Optimized Builds** - LTO and optimization in release mode
- **Minimal Dependencies** - Only essential dependencies

## License

MIT License - feel free to use in your projects

## Contributing

Contributions welcome! Please open issues or pull requests.

## Support

For issues and questions, please open a GitHub issue.
