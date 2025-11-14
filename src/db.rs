use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Clone)]
pub struct Db {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mailbox {
    pub id: Uuid,
    pub local: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>, // <-- Added field for TTL logic
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub mailbox_id: Uuid,
    // FIX E0599 (unwrap_or_else): from_addr must be Option<String> for unwrap_or_else to work
    pub from_addr: Option<String>, 
    pub to_addr: String,
    pub subject: String,
    pub body_text: String,
    pub body_html: Option<String>,
    pub raw: String, // raw_email was renamed to raw for simplicity
    pub received_at: DateTime<Utc>,
}

impl Db {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPool::connect(database_url).await?;
        Ok(Self { pool })
    }

    pub async fn run_migrations(&self) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS mailboxes (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                local VARCHAR(255) NOT NULL UNIQUE,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                expires_at TIMESTAMPTZ -- Added column
            );

            CREATE INDEX IF NOT EXISTS idx_mailboxes_local ON mailboxes(local);
            CREATE INDEX IF NOT EXISTS idx_mailboxes_expires_at ON mailboxes(expires_at); -- Added index

            CREATE TABLE IF NOT EXISTS messages (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
                from_addr TEXT, -- Changed to allow NULL to match Option<String>
                to_addr TEXT NOT NULL,
                subject TEXT NOT NULL,
                body_text TEXT NOT NULL,
                body_html TEXT,
                raw TEXT NOT NULL, -- Renamed raw_email to raw
                received_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_messages_mailbox_id ON messages(mailbox_id);
            CREATE INDEX IF NOT EXISTS idx_messages_received_at ON messages(received_at DESC);
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    // FIX E0061: Updated signature to accept ttl_seconds
    pub async fn create_mailbox(&self, local: &str, ttl_seconds: Option<i64>) -> Result<Mailbox> {
        let query = if ttl_seconds.is_some() {
            // Use an SQL expression to calculate expires_at
            sqlx::query(
                "INSERT INTO mailboxes (local, expires_at) VALUES ($1, NOW() + INTERVAL '1 second' * $2) RETURNING id, local, created_at, expires_at"
            )
            .bind(local)
            .bind(ttl_seconds.unwrap_or(0))
        } else {
            sqlx::query(
                "INSERT INTO mailboxes (local) VALUES ($1) RETURNING id, local, created_at, expires_at"
            )
            .bind(local)
        };

        let row = query.fetch_one(&self.pool).await?;

        Ok(Mailbox {
            id: row.get("id"),
            local: row.get("local"),
            created_at: row.get("created_at"),
            expires_at: row.get("expires_at"),
        })
    }
    
    // Helper to get Mailbox ID
    pub async fn get_mailbox_by_local(&self, local: &str) -> Result<Option<Mailbox>> {
        let row = sqlx::query("SELECT id, local, created_at, expires_at FROM mailboxes WHERE local = $1")
            .bind(local)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| Mailbox {
            id: r.get("id"),
            local: r.get("local"),
            created_at: r.get("created_at"),
            expires_at: r.get("expires_at"),
        }))
    }

    // FIX E0599: Implementation of mailbox_exists
    pub async fn mailbox_exists(&self, local: &str) -> Result<bool> {
        let opt = self.get_mailbox_by_local(local).await?;
        Ok(opt.is_some())
    }

    // FIX E0599: Implementation of list_messages
    pub async fn list_messages(&self, local: &str) -> Result<Vec<Message>> {
        let mailbox = match self.get_mailbox_by_local(local).await? {
            Some(m) => m,
            None => return Ok(vec![]),
        };

        let rows = sqlx::query(
            r#"
            SELECT id, mailbox_id, from_addr, to_addr, subject, body_text, body_html, raw, received_at
            FROM messages
            WHERE mailbox_id = $1
            ORDER BY received_at DESC
            "#
        )
        .bind(mailbox.id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| Message {
                id: r.get("id"),
                mailbox_id: r.get("mailbox_id"),
                from_addr: r.get("from_addr"),
                to_addr: r.get("to_addr"),
                subject: r.get("subject"),
                body_text: r.get("body_text"),
                body_html: r.get("body_html"),
                raw: r.get("raw"), // Changed from raw_email
                received_at: r.get("received_at"),
            })
            .collect())
    }

    // FIX E0599: Implementation of get_message (renamed from get_message_by_id)
    // The HTTP handler passes `local` and `uuid`, so the DB method must use both for validation.
    pub async fn get_message(&self, local: &str, id: Uuid) -> Result<Option<Message>> {
        let mailbox = match self.get_mailbox_by_local(local).await? {
            Some(m) => m,
            None => return Ok(None),
        };

        let row = sqlx::query(
            r#"
            SELECT id, mailbox_id, from_addr, to_addr, subject, body_text, body_html, raw, received_at
            FROM messages
            WHERE id = $1 AND mailbox_id = $2
            "#
        )
        .bind(id)
        .bind(mailbox.id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| Message {
            id: r.get("id"),
            mailbox_id: r.get("mailbox_id"),
            from_addr: r.get("from_addr"),
            to_addr: r.get("to_addr"),
            subject: r.get("subject"),
            body_text: r.get("body_text"),
            body_html: r.get("body_html"),
            raw: r.get("raw"),
            received_at: r.get("received_at"),
        }))
    }
    
    // ... (other functions from db.rs, like create_message, delete_old_messages, etc.)
    // Note: I've updated create_message to use raw instead of raw_email and from_addr as Option<String>

    pub async fn create_message(
        &self,
        mailbox_id: Uuid,
        from_addr: Option<&str>, // Changed to Option
        to_addr: &str,
        subject: &str,
        body_text: &str,
        body_html: Option<&str>,
        raw_email: &str, // Renaming this to 'raw' in usage
    ) -> Result<Message> {
        let row = sqlx::query(
            r#"
            INSERT INTO messages (mailbox_id, from_addr, to_addr, subject, body_text, body_html, raw)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, mailbox_id, from_addr, to_addr, subject, body_text, body_html, raw, received_at
            "#
        )
        .bind(mailbox_id)
        .bind(from_addr)
        .bind(to_addr)
        .bind(subject)
        .bind(body_text)
        .bind(body_html)
        .bind(raw_email)
        .fetch_one(&self.pool)
        .await?;

        Ok(Message {
            id: row.get("id"),
            mailbox_id: row.get("mailbox_id"),
            from_addr: row.get("from_addr"),
            to_addr: row.get("to_addr"),
            subject: row.get("subject"),
            body_text: row.get("body_text"),
            body_html: row.get("body_html"),
            raw: row.get("raw"), // Changed from raw_email
            received_at: row.get("received_at"),
        })
    }
    
    // ... (rest of the Db impl unchanged)
    pub async fn delete_old_messages(&self, days: i64) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM messages WHERE received_at < NOW() - INTERVAL '1 day' * $1"
        )
        .bind(days)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    pub async fn delete_old_mailboxes(&self, days: i64) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM mailboxes WHERE created_at < NOW() - INTERVAL '1 day' * $1"
        )
        .bind(days)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}
