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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub mailbox_id: Uuid,
    pub from_addr: String,
    pub to_addr: String,
    pub subject: String,
    pub body_text: String,
    pub body_html: Option<String>,
    pub raw_email: String,
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
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_mailboxes_local ON mailboxes(local);

            CREATE TABLE IF NOT EXISTS messages (
                id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
                mailbox_id UUID NOT NULL REFERENCES mailboxes(id) ON DELETE CASCADE,
                from_addr TEXT NOT NULL,
                to_addr TEXT NOT NULL,
                subject TEXT NOT NULL,
                body_text TEXT NOT NULL,
                body_html TEXT,
                raw_email TEXT NOT NULL,
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

    pub async fn create_mailbox(&self, local: &str) -> Result<Mailbox> {
        let row = sqlx::query(
            "INSERT INTO mailboxes (local) VALUES ($1) RETURNING id, local, created_at"
        )
        .bind(local)
        .fetch_one(&self.pool)
        .await?;

        Ok(Mailbox {
            id: row.get("id"),
            local: row.get("local"),
            created_at: row.get("created_at"),
        })
    }

    pub async fn get_mailbox_by_local(&self, local: &str) -> Result<Option<Mailbox>> {
        let row = sqlx::query("SELECT id, local, created_at FROM mailboxes WHERE local = $1")
            .bind(local)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|r| Mailbox {
            id: r.get("id"),
            local: r.get("local"),
            created_at: r.get("created_at"),
        }))
    }

    pub async fn create_message(
        &self,
        mailbox_id: Uuid,
        from_addr: &str,
        to_addr: &str,
        subject: &str,
        body_text: &str,
        body_html: Option<&str>,
        raw_email: &str,
    ) -> Result<Message> {
        let row = sqlx::query(
            r#"
            INSERT INTO messages (mailbox_id, from_addr, to_addr, subject, body_text, body_html, raw_email)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, mailbox_id, from_addr, to_addr, subject, body_text, body_html, raw_email, received_at
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
            raw_email: row.get("raw_email"),
            received_at: row.get("received_at"),
        })
    }

    pub async fn get_messages_by_mailbox(&self, mailbox_id: Uuid) -> Result<Vec<Message>> {
        let rows = sqlx::query(
            r#"
            SELECT id, mailbox_id, from_addr, to_addr, subject, body_text, body_html, raw_email, received_at
            FROM messages
            WHERE mailbox_id = $1
            ORDER BY received_at DESC
            "#
        )
        .bind(mailbox_id)
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
                raw_email: r.get("raw_email"),
                received_at: r.get("received_at"),
            })
            .collect())
    }

    pub async fn get_message_by_id(&self, id: Uuid) -> Result<Option<Message>> {
        let row = sqlx::query(
            r#"
            SELECT id, mailbox_id, from_addr, to_addr, subject, body_text, body_html, raw_email, received_at
            FROM messages
            WHERE id = $1
            "#
        )
        .bind(id)
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
            raw_email: r.get("raw_email"),
            received_at: r.get("received_at"),
        }))
    }

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
