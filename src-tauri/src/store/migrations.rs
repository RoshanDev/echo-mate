// Database migrations for SQLite/SQLCipher

pub fn run_migrations(conn: &rusqlite::Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS memory_item (
            id TEXT PRIMARY KEY,
            type TEXT NOT NULL,
            value TEXT NOT NULL,
            source_kind TEXT NOT NULL,
            source_ref TEXT NOT NULL,
            source_excerpt TEXT NOT NULL,
            confidence REAL NOT NULL,
            sensitivity TEXT NOT NULL,
            expires_at TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS reminder (
            id TEXT PRIMARY KEY,
            memory_id TEXT NOT NULL,
            trigger_at TEXT NOT NULL,
            reason TEXT NOT NULL,
            suggested_follow_up TEXT NOT NULL,
            status TEXT NOT NULL,
            snooze_count INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(memory_id) REFERENCES memory_item(id)
        );

        CREATE INDEX IF NOT EXISTS idx_reminder_status_trigger
            ON reminder(status, trigger_at);

        CREATE TABLE IF NOT EXISTS context_summary (
            id TEXT PRIMARY KEY,
            source_kind TEXT NOT NULL,
            source_ref TEXT NOT NULL,
            summary TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS reply_feedback (
            id TEXT PRIMARY KEY,
            generation_id TEXT NOT NULL,
            action TEXT NOT NULL,
            candidate_index INTEGER NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_reply_feedback_created
            ON reply_feedback(created_at);
        "#,
    )?;
    Ok(())
}
