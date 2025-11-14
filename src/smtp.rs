use crate::db::Db;
use anyhow::{Context, Result};
use mail_parser::MessageParser;
use std::net::SocketAddr;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

pub async fn start_server(addr: SocketAddr, domain: String, db: Db) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    tracing::info!("SMTP server listening on {}", addr);

    loop {
        match listener.accept().await {
            Ok((stream, peer)) => {
                let domain = domain.clone();
                let db = db.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, peer, &domain, db).await {
                        tracing::error!("Connection error from {}: {}", peer, e);
                    }
                });
            }
            Err(e) => {
                tracing::error!("Failed to accept connection: {}", e);
            }
        }
    }
}

async fn handle_connection(
    stream: TcpStream,
    _peer: SocketAddr,
    domain: &str,
    db: Db,
) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    // Send greeting
    writer
        .write_all(format!("220 {} ESMTP Temporary Mail Server\r\n", domain).as_bytes())
        .await?;

    let mut mail_from = String::new();
    let mut rcpt_to = Vec::new();
    let mut data_buffer = Vec::new();

    loop {
        line.clear();
        let bytes_read = reader.read_line(&mut line).await?;
        
        if bytes_read == 0 {
            break;
        }

        let command = line.trim();
        tracing::debug!("Received: {}", command);

        let parts: Vec<&str> = command.splitn(2, ' ').collect();
        let cmd = parts[0].to_uppercase();

        match cmd.as_str() {
            "HELO" | "EHLO" => {
                writer
                    .write_all(format!("250-{} Hello\r\n", domain).as_bytes())
                    .await?;
                writer.write_all(b"250-SIZE 10485760\r\n").await?;
                writer.write_all(b"250-8BITMIME\r\n").await?;
                writer.write_all(b"250 PIPELINING\r\n").await?;
            }
            "MAIL" => {
                if let Some(from) = extract_email(command) {
                    mail_from = from;
                    writer.write_all(b"250 OK\r\n").await?;
                } else {
                    writer.write_all(b"501 Syntax error\r\n").await?;
                }
            }
            "RCPT" => {
                if let Some(to) = extract_email(command) {
                    // Check if mailbox exists or domain matches
                    let local = to.split('@').next().unwrap_or("");
                    if to.ends_with(&format!("@{}", domain)) {
                        rcpt_to.push(to);
                        writer.write_all(b"250 OK\r\n").await?;
                    } else {
                        writer
                            .write_all(b"550 Mailbox unavailable\r\n")
                            .await?;
                    }
                } else {
                    writer.write_all(b"501 Syntax error\r\n").await?;
                }
            }
            "DATA" => {
                if mail_from.is_empty() || rcpt_to.is_empty() {
                    writer
                        .write_all(b"503 Bad sequence of commands\r\n")
                        .await?;
                    continue;
                }

                writer
                    .write_all(b"354 Start mail input; end with <CRLF>.<CRLF>\r\n")
                    .await?;

                data_buffer.clear();
                loop {
                    line.clear();
                    reader.read_line(&mut line).await?;

                    if line == ".\r\n" || line == ".\n" {
                        break;
                    }

                    data_buffer.extend_from_slice(line.as_bytes());
                }

                // Process the email
                match process_email(&db, &mail_from, &rcpt_to, &data_buffer, domain).await {
                    Ok(_) => {
                        writer.write_all(b"250 OK: Message accepted\r\n").await?;
                    }
                    Err(e) => {
                        tracing::error!("Failed to process email: {}", e);
                        writer
                            .write_all(b"451 Temporary failure\r\n")
                            .await?;
                    }
                }

                mail_from.clear();
                rcpt_to.clear();
            }
            "RSET" => {
                mail_from.clear();
                rcpt_to.clear();
                data_buffer.clear();
                writer.write_all(b"250 OK\r\n").await?;
            }
            "QUIT" => {
                writer.write_all(b"221 Bye\r\n").await?;
                break;
            }
            "NOOP" => {
                writer.write_all(b"250 OK\r\n").await?;
            }
            _ => {
                writer
                    .write_all(b"502 Command not implemented\r\n")
                    .await?;
            }
        }
    }

    Ok(())
}

fn extract_email(command: &str) -> Option<String> {
    let start = command.find('<')?;
    let end = command.find('>')?;
    Some(command[start + 1..end].to_lowercase())
}

async fn process_email(
    db: &Db,
    from: &str,
    recipients: &[String],
    raw_data: &[u8],
    domain: &str,
) -> Result<()> {
    let raw_email = String::from_utf8_lossy(raw_data).to_string();
    
    // Parse email
    let parser = MessageParser::default();
    let message = parser
        .parse(raw_data)
        .context("Failed to parse email")?;

    let subject = message
        .subject()
        .unwrap_or("(No Subject)")
        .to_string();

    let body_text = message
        .body_text(0)
        .map(|s| s.to_string())
        .unwrap_or_else(|| "(No text body)".to_string());

    let body_html = message.body_html(0).map(|s| s.to_string());

    // Store message for each recipient
    for recipient in recipients {
        if !recipient.ends_with(&format!("@{}", domain)) {
            continue;
        }

        let local = recipient.split('@').next().unwrap_or("");
        
        // Get or create mailbox
        let mailbox = match db.get_mailbox_by_local(local).await? {
            Some(mb) => mb,
            None => {
                db.create_mailbox(local,None).await?
            }
        };

        // Store message
        db.create_message(
            mailbox.id,
            Some(from),
            recipient,
            &subject,
            &body_text,
            body_html.as_deref(),
            &raw_email,
        )
        .await?;

        tracing::info!("Email stored for {}: {}", recipient, subject);
    }

    Ok(())
}
