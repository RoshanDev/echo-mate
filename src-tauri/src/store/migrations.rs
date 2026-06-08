// Database migrations for SQLite/SQLCipher

pub fn run_migrations(conn: &rusqlite::Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS contacts (
            id TEXT PRIMARY KEY,
            alias TEXT NOT NULL,
            channel TEXT NOT NULL,
            is_allowlisted INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_contacts_alias_channel
            ON contacts(alias, channel);

        CREATE TABLE IF NOT EXISTS memory_item (
            id TEXT PRIMARY KEY,
            contact_id TEXT NOT NULL DEFAULT '',
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

        CREATE INDEX IF NOT EXISTS idx_memory_item_contact_status
            ON memory_item(contact_id, status, updated_at);

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
            contact_id TEXT NOT NULL DEFAULT '',
            source_kind TEXT NOT NULL,
            source_ref TEXT NOT NULL,
            summary TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_context_summary_contact_created
            ON context_summary(contact_id, created_at);

        CREATE TABLE IF NOT EXISTS reply_feedback (
            id TEXT PRIMARY KEY,
            generation_id TEXT NOT NULL,
            action TEXT NOT NULL,
            candidate_index INTEGER NOT NULL,
            candidate_text TEXT NOT NULL DEFAULT '',
            contact_id TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_reply_feedback_created
            ON reply_feedback(created_at);

        CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            contact_id TEXT NOT NULL,
            role TEXT NOT NULL,
            text TEXT NOT NULL,
            source TEXT NOT NULL,
            approved INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            FOREIGN KEY(contact_id) REFERENCES contacts(id)
        );

        CREATE INDEX IF NOT EXISTS idx_messages_contact_created
            ON messages(contact_id, created_at);

        CREATE TABLE IF NOT EXISTS style_profile (
            id TEXT PRIMARY KEY,
            profile_json TEXT NOT NULL,
            sample_count INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS platform_signal_log (
            id TEXT PRIMARY KEY,
            contact_id TEXT NOT NULL DEFAULT '',
            contact_alias TEXT NOT NULL DEFAULT '',
            channel TEXT NOT NULL DEFAULT 'wechat',
            source TEXT NOT NULL,
            app_name TEXT NOT NULL DEFAULT '',
            text_excerpt TEXT NOT NULL DEFAULT '',
            allowed INTEGER NOT NULL DEFAULT 0,
            reason TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_platform_signal_log_contact_created
            ON platform_signal_log(contact_id, created_at);
        "#,
    )?;
    add_column_if_missing(
        conn,
        "memory_item",
        "contact_id",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        conn,
        "context_summary",
        "contact_id",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        conn,
        "reply_feedback",
        "candidate_text",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        conn,
        "reply_feedback",
        "contact_id",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    Ok(())
}

fn add_column_if_missing(
    conn: &rusqlite::Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> anyhow::Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;
    for existing in columns {
        if existing? == column {
            return Ok(());
        }
    }
    conn.execute_batch(&format!(
        "ALTER TABLE {table} ADD COLUMN {column} {definition}"
    ))?;
    Ok(())
}
