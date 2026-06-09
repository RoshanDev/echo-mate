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
            contact_id TEXT NOT NULL DEFAULT '',
            kind TEXT NOT NULL DEFAULT 'follow_up',
            due_at TEXT NOT NULL DEFAULT '',
            trigger_at TEXT NOT NULL,
            reason TEXT NOT NULL,
            suggested_follow_up TEXT NOT NULL,
            source_memory_id TEXT NOT NULL DEFAULT '',
            source_context_id TEXT NOT NULL DEFAULT '',
            cooldown_key TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL,
            snooze_until TEXT NOT NULL DEFAULT '',
            snooze_count INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(memory_id) REFERENCES memory_item(id)
        );

        CREATE INDEX IF NOT EXISTS idx_reminder_status_trigger
            ON reminder(status, trigger_at);

        CREATE TABLE IF NOT EXISTS reminder_mutes (
            id TEXT PRIMARY KEY,
            contact_id TEXT NOT NULL DEFAULT '',
            kind TEXT NOT NULL DEFAULT '',
            muted_until TEXT NOT NULL DEFAULT '',
            reason TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_reminder_mutes_contact_kind
            ON reminder_mutes(contact_id, kind, muted_until);

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

        CREATE TABLE IF NOT EXISTS source_contexts (
            id TEXT PRIMARY KEY,
            contact_id TEXT NOT NULL DEFAULT '',
            provider TEXT NOT NULL DEFAULT '',
            input_kind TEXT NOT NULL,
            fact_source TEXT NOT NULL DEFAULT '',
            source_label TEXT NOT NULL DEFAULT '',
            source_excerpt TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            captured_at TEXT NOT NULL DEFAULT '',
            visible_message_time TEXT NOT NULL DEFAULT '',
            inferred_chat_time TEXT NOT NULL DEFAULT '',
            source_confidence REAL NOT NULL DEFAULT 0,
            metadata_json TEXT NOT NULL DEFAULT '{}',
            FOREIGN KEY(contact_id) REFERENCES contacts(id)
        );

        CREATE INDEX IF NOT EXISTS idx_source_contexts_contact_created
            ON source_contexts(contact_id, created_at);

        CREATE TABLE IF NOT EXISTS message_events (
            id TEXT PRIMARY KEY,
            message_id TEXT NOT NULL DEFAULT '',
            contact_id TEXT NOT NULL DEFAULT '',
            role TEXT NOT NULL DEFAULT '',
            text_excerpt TEXT NOT NULL DEFAULT '',
            provider TEXT NOT NULL DEFAULT '',
            input_kind TEXT NOT NULL DEFAULT '',
            fact_source TEXT NOT NULL DEFAULT '',
            source_context_id TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            captured_at TEXT NOT NULL DEFAULT '',
            visible_message_time TEXT NOT NULL DEFAULT '',
            inferred_chat_time TEXT NOT NULL DEFAULT '',
            source_confidence REAL NOT NULL DEFAULT 0,
            FOREIGN KEY(contact_id) REFERENCES contacts(id)
        );

        CREATE INDEX IF NOT EXISTS idx_message_events_contact_created
            ON message_events(contact_id, created_at);

        CREATE TABLE IF NOT EXISTS screenshot_analyses (
            id TEXT PRIMARY KEY,
            contact_id TEXT NOT NULL DEFAULT '',
            source_context_id TEXT NOT NULL DEFAULT '',
            image_path TEXT NOT NULL DEFAULT '',
            image_width INTEGER NOT NULL DEFAULT 0,
            image_height INTEGER NOT NULL DEFAULT 0,
            parser_version TEXT NOT NULL DEFAULT '',
            ocr_provider TEXT NOT NULL DEFAULT '',
            turns_json TEXT NOT NULL DEFAULT '[]',
            last_reply_target TEXT NOT NULL DEFAULT '',
            visible_time_label TEXT NOT NULL DEFAULT '',
            inferred_chat_time TEXT NOT NULL DEFAULT '',
            staleness TEXT NOT NULL DEFAULT 'unknown',
            warnings_json TEXT NOT NULL DEFAULT '[]',
            confidence REAL NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            FOREIGN KEY(contact_id) REFERENCES contacts(id)
        );

        CREATE INDEX IF NOT EXISTS idx_screenshot_analyses_contact_created
            ON screenshot_analyses(contact_id, created_at);

        CREATE TABLE IF NOT EXISTS suggestion_runs (
            id TEXT PRIMARY KEY,
            contact_id TEXT NOT NULL DEFAULT '',
            provider TEXT NOT NULL DEFAULT '',
            input_kind TEXT NOT NULL,
            fact_source TEXT NOT NULL DEFAULT '',
            source_context_id TEXT NOT NULL DEFAULT '',
            context_sources_json TEXT NOT NULL DEFAULT '[]',
            output_summary TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            captured_at TEXT NOT NULL DEFAULT '',
            visible_message_time TEXT NOT NULL DEFAULT '',
            inferred_chat_time TEXT NOT NULL DEFAULT '',
            source_confidence REAL NOT NULL DEFAULT 0,
            FOREIGN KEY(contact_id) REFERENCES contacts(id)
        );

        CREATE INDEX IF NOT EXISTS idx_suggestion_runs_contact_created
            ON suggestion_runs(contact_id, created_at);

        CREATE TABLE IF NOT EXISTS memory_candidates (
            id TEXT PRIMARY KEY,
            contact_id TEXT NOT NULL DEFAULT '',
            suggestion_run_id TEXT NOT NULL DEFAULT '',
            source_context_id TEXT NOT NULL DEFAULT '',
            candidate_index INTEGER NOT NULL DEFAULT 0,
            memory_type TEXT NOT NULL DEFAULT '',
            summary TEXT NOT NULL DEFAULT '',
            value TEXT NOT NULL DEFAULT '',
            source_kind TEXT NOT NULL DEFAULT '',
            source_ref TEXT NOT NULL DEFAULT '',
            source_excerpt TEXT NOT NULL DEFAULT '',
            source_quote TEXT NOT NULL DEFAULT '',
            reason TEXT NOT NULL DEFAULT '',
            fact_source TEXT NOT NULL DEFAULT '',
            confidence REAL NOT NULL DEFAULT 0,
            sensitivity TEXT NOT NULL DEFAULT '',
            expires_at TEXT NOT NULL DEFAULT '',
            ttl_days INTEGER,
            status TEXT NOT NULL DEFAULT 'candidate',
            created_at TEXT NOT NULL,
            captured_at TEXT NOT NULL DEFAULT '',
            visible_message_time TEXT NOT NULL DEFAULT '',
            inferred_chat_time TEXT NOT NULL DEFAULT '',
            source_confidence REAL NOT NULL DEFAULT 0,
            FOREIGN KEY(contact_id) REFERENCES contacts(id)
        );

        CREATE INDEX IF NOT EXISTS idx_memory_candidates_contact_created
            ON memory_candidates(contact_id, created_at);

        CREATE TABLE IF NOT EXISTS contact_facts (
            id TEXT PRIMARY KEY,
            contact_id TEXT NOT NULL,
            fact_type TEXT NOT NULL,
            value TEXT NOT NULL,
            normalized_value TEXT NOT NULL DEFAULT '',
            source_note TEXT NOT NULL DEFAULT '',
            provider TEXT NOT NULL DEFAULT '',
            input_kind TEXT NOT NULL DEFAULT 'manual',
            fact_source TEXT NOT NULL DEFAULT 'manual',
            sensitivity TEXT NOT NULL DEFAULT 'normal',
            confidence REAL NOT NULL DEFAULT 0,
            ttl_days INTEGER,
            usage_policy TEXT NOT NULL DEFAULT 'contextual',
            source_context_id TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL,
            captured_at TEXT NOT NULL DEFAULT '',
            visible_message_time TEXT NOT NULL DEFAULT '',
            inferred_chat_time TEXT NOT NULL DEFAULT '',
            source_confidence REAL NOT NULL DEFAULT 0,
            updated_at TEXT NOT NULL,
            last_used_at TEXT NOT NULL DEFAULT '',
            FOREIGN KEY(contact_id) REFERENCES contacts(id)
        );

        CREATE INDEX IF NOT EXISTS idx_contact_facts_contact_updated
            ON contact_facts(contact_id, updated_at);

        CREATE UNIQUE INDEX IF NOT EXISTS idx_contact_facts_unique
            ON contact_facts(contact_id, fact_type, normalized_value, fact_source);
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
    add_column_if_missing(
        conn,
        "messages",
        "source_context_id",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(conn, "messages", "captured_at", "TEXT NOT NULL DEFAULT ''")?;
    add_column_if_missing(
        conn,
        "messages",
        "visible_message_time",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        conn,
        "messages",
        "inferred_chat_time",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        conn,
        "messages",
        "source_confidence",
        "REAL NOT NULL DEFAULT 0",
    )?;
    add_column_if_missing(
        conn,
        "context_summary",
        "source_context_id",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        conn,
        "context_summary",
        "captured_at",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        conn,
        "context_summary",
        "visible_message_time",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        conn,
        "context_summary",
        "inferred_chat_time",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        conn,
        "context_summary",
        "source_confidence",
        "REAL NOT NULL DEFAULT 0",
    )?;
    add_column_if_missing(
        conn,
        "memory_item",
        "fact_source",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        conn,
        "memory_item",
        "source_context_id",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        conn,
        "memory_item",
        "last_used_at",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(conn, "reminder", "contact_id", "TEXT NOT NULL DEFAULT ''")?;
    add_column_if_missing(
        conn,
        "reminder",
        "kind",
        "TEXT NOT NULL DEFAULT 'follow_up'",
    )?;
    add_column_if_missing(conn, "reminder", "due_at", "TEXT NOT NULL DEFAULT ''")?;
    add_column_if_missing(
        conn,
        "reminder",
        "source_context_id",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        conn,
        "reminder",
        "source_memory_id",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(conn, "reminder", "cooldown_key", "TEXT NOT NULL DEFAULT ''")?;
    add_column_if_missing(conn, "reminder", "snooze_until", "TEXT NOT NULL DEFAULT ''")?;
    add_column_if_missing(
        conn,
        "memory_candidates",
        "summary",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        conn,
        "memory_candidates",
        "source_quote",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        conn,
        "memory_candidates",
        "reason",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(conn, "memory_candidates", "ttl_days", "INTEGER")?;
    add_column_if_missing(
        conn,
        "reply_feedback",
        "suggestion_run_id",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        conn,
        "suggestion_runs",
        "fact_source",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        conn,
        "suggestion_runs",
        "captured_at",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        conn,
        "suggestion_runs",
        "visible_message_time",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        conn,
        "suggestion_runs",
        "inferred_chat_time",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        conn,
        "suggestion_runs",
        "source_confidence",
        "REAL NOT NULL DEFAULT 0",
    )?;
    add_column_if_missing(
        conn,
        "contact_facts",
        "provider",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        conn,
        "contact_facts",
        "input_kind",
        "TEXT NOT NULL DEFAULT 'manual'",
    )?;
    add_column_if_missing(
        conn,
        "contact_facts",
        "source_context_id",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        conn,
        "contact_facts",
        "captured_at",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        conn,
        "contact_facts",
        "visible_message_time",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        conn,
        "contact_facts",
        "inferred_chat_time",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    add_column_if_missing(
        conn,
        "contact_facts",
        "source_confidence",
        "REAL NOT NULL DEFAULT 0",
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
