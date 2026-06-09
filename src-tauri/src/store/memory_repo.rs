// Memory repository for style profiles, contact facts, and reminder MVP data.
use crate::domain::{
    Candidate, ContactInput, ContactRecord, ContextSummaryCandidate, ContextSummaryRecord,
    MemoryCandidate, MemoryItemRecord, MessageRecord, NextAction, ReminderCandidate,
    ReminderDetail, ReminderRecord, ReplyFeedbackRecord, StyleProfileRecord,
};
use crate::store::migrations::run_migrations;
use anyhow::{anyhow, bail};
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct MemoryRepository {
    db_path: PathBuf,
}

const STYLE_PROFILE_REBUILD_LIMIT: i64 = 200;

impl MemoryRepository {
    pub fn open_default() -> anyhow::Result<Self> {
        Self::new(default_db_path())
    }

    pub fn new(db_path: PathBuf) -> anyhow::Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&db_path)?;
        run_migrations(&conn)?;
        Ok(Self { db_path })
    }

    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    fn connection(&self) -> anyhow::Result<Connection> {
        let conn = Connection::open(&self.db_path)?;
        run_migrations(&conn)?;
        Ok(conn)
    }

    pub fn list_contacts(&self) -> anyhow::Result<Vec<ContactRecord>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, alias, channel, is_allowlisted, created_at, updated_at
             FROM contacts
             ORDER BY updated_at DESC, alias ASC",
        )?;
        let rows = stmt.query_map([], contact_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn upsert_contact(&self, input: &ContactInput) -> anyhow::Result<ContactRecord> {
        let alias = input.alias.trim();
        if alias.is_empty() {
            bail!("联系人别名不能为空");
        }
        let channel = non_empty(&input.channel, "wechat");
        let now = now_rfc3339();
        let conn = self.connection()?;
        let existing = input
            .id
            .as_deref()
            .filter(|id| !id.trim().is_empty())
            .and_then(|id| self.get_contact(id).ok().flatten());
        let existing_by_key = if existing.is_none() {
            self.find_contact_by_alias_channel(alias, &channel)?
        } else {
            None
        };
        let id = existing
            .or(existing_by_key)
            .map(|contact| contact.id)
            .unwrap_or_else(|| next_id("contact"));

        conn.execute(
            "INSERT INTO contacts (id, alias, channel, is_allowlisted, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                alias = excluded.alias,
                channel = excluded.channel,
                is_allowlisted = excluded.is_allowlisted,
                updated_at = excluded.updated_at",
            params![
                &id,
                alias,
                &channel,
                bool_to_i64(input.is_allowlisted),
                &now,
                &now
            ],
        )?;
        self.get_contact(&id)?
            .ok_or_else(|| anyhow!("联系人保存后未能读取"))
    }

    pub fn get_contact(&self, id: &str) -> anyhow::Result<Option<ContactRecord>> {
        let conn = self.connection()?;
        conn.query_row(
            "SELECT id, alias, channel, is_allowlisted, created_at, updated_at
             FROM contacts WHERE id = ?1",
            params![id],
            contact_from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn find_allowlisted_contact(
        &self,
        alias: &str,
        channel: &str,
    ) -> anyhow::Result<Option<ContactRecord>> {
        let conn = self.connection()?;
        conn.query_row(
            "SELECT id, alias, channel, is_allowlisted, created_at, updated_at
             FROM contacts
             WHERE lower(alias) = lower(?1)
               AND lower(channel) = lower(?2)
               AND is_allowlisted = 1
             LIMIT 1",
            params![alias.trim(), non_empty(channel, "wechat")],
            contact_from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    fn find_contact_by_alias_channel(
        &self,
        alias: &str,
        channel: &str,
    ) -> anyhow::Result<Option<ContactRecord>> {
        let conn = self.connection()?;
        conn.query_row(
            "SELECT id, alias, channel, is_allowlisted, created_at, updated_at
             FROM contacts
             WHERE lower(alias) = lower(?1)
               AND lower(channel) = lower(?2)
             LIMIT 1",
            params![alias.trim(), non_empty(channel, "wechat")],
            contact_from_row,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn delete_contact(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.connection()?;
        conn.execute("DELETE FROM messages WHERE contact_id = ?1", params![id])?;
        conn.execute(
            "DELETE FROM platform_signal_log WHERE contact_id = ?1",
            params![id],
        )?;
        conn.execute(
            "DELETE FROM context_summary WHERE contact_id = ?1",
            params![id],
        )?;
        conn.execute(
            "UPDATE memory_item SET status = 'deleted', updated_at = ?2 WHERE contact_id = ?1",
            params![id, now_rfc3339()],
        )?;
        conn.execute(
            "UPDATE reminder
             SET status = 'cancelled', updated_at = ?2
             WHERE memory_id IN (SELECT id FROM memory_item WHERE contact_id = ?1)",
            params![id, now_rfc3339()],
        )?;
        conn.execute("DELETE FROM contacts WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn clear_contact_context(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.connection()?;
        conn.execute("DELETE FROM messages WHERE contact_id = ?1", params![id])?;
        conn.execute(
            "DELETE FROM reply_feedback WHERE contact_id = ?1",
            params![id],
        )?;
        conn.execute(
            "DELETE FROM platform_signal_log WHERE contact_id = ?1",
            params![id],
        )?;
        conn.execute(
            "DELETE FROM context_summary WHERE contact_id = ?1",
            params![id],
        )?;
        conn.execute(
            "UPDATE memory_item SET status = 'deleted', updated_at = ?2 WHERE contact_id = ?1",
            params![id, now_rfc3339()],
        )?;
        conn.execute(
            "UPDATE reminder
             SET status = 'cancelled', updated_at = ?2
             WHERE memory_id IN (SELECT id FROM memory_item WHERE contact_id = ?1)",
            params![id, now_rfc3339()],
        )?;
        Ok(())
    }

    pub fn append_message(
        &self,
        contact_id: &str,
        role: &str,
        text: &str,
        source: &str,
        approved: bool,
    ) -> anyhow::Result<MessageRecord> {
        if contact_id.trim().is_empty() {
            bail!("contact_id is required to save a message");
        }
        if text.trim().is_empty() {
            bail!("message text is empty");
        }
        let record = MessageRecord {
            id: next_id("msg"),
            contact_id: contact_id.to_string(),
            role: non_empty(role, "other"),
            text: text.trim().chars().take(800).collect(),
            source: non_empty(source, "manual"),
            approved,
            created_at: now_rfc3339(),
        };
        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO messages (id, contact_id, role, text, source, approved, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                &record.id,
                &record.contact_id,
                &record.role,
                &record.text,
                &record.source,
                bool_to_i64(record.approved),
                &record.created_at
            ],
        )?;
        Ok(record)
    }

    pub fn record_platform_signal_log(
        &self,
        contact_id: &str,
        contact_alias: &str,
        channel: &str,
        source: &str,
        app_name: &str,
        text: &str,
        allowed: bool,
        reason: &str,
    ) -> anyhow::Result<()> {
        if contact_id.trim().is_empty() {
            bail!("contact_id is required to save a platform signal log");
        }
        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO platform_signal_log
                (id, contact_id, contact_alias, channel, source, app_name, text_excerpt, allowed, reason, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                next_id("sig"),
                contact_id,
                contact_alias.trim(),
                non_empty(channel, "wechat"),
                non_empty(source, "notification"),
                app_name.trim(),
                truncate_chars(text.trim(), 200),
                bool_to_i64(allowed),
                reason,
                now_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn platform_signal_log_count(&self, contact_id: &str) -> anyhow::Result<i64> {
        let conn = self.connection()?;
        conn.query_row(
            "SELECT COUNT(*) FROM platform_signal_log WHERE contact_id = ?1",
            params![contact_id],
            |row| row.get(0),
        )
        .map_err(Into::into)
    }

    pub fn recent_messages(
        &self,
        contact_id: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<MessageRecord>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, contact_id, role, text, source, approved, created_at
             FROM messages
             WHERE contact_id = ?1
             ORDER BY created_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![contact_id, limit as i64], message_from_row)?;
        let mut messages = rows.collect::<Result<Vec<_>, _>>()?;
        messages.reverse();
        Ok(messages)
    }

    pub fn confirmed_memories_for_contact(
        &self,
        contact_id: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<MemoryItemRecord>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, contact_id, type, value, source_kind, source_ref, source_excerpt,
                confidence, sensitivity, expires_at, status, created_at, updated_at
             FROM memory_item
             WHERE contact_id = ?1 AND status = 'confirmed'
             ORDER BY updated_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![contact_id, limit as i64], memory_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn update_style_profile_from_reply(
        &self,
        adopted_text: &str,
    ) -> anyhow::Result<StyleProfileRecord> {
        let text = adopted_text.trim();
        if text.is_empty() {
            bail!("adopted reply is empty");
        }
        let existing = self.style_profile()?;
        let sample_count = existing
            .as_ref()
            .map(|profile| profile.sample_count + 1)
            .unwrap_or(1);
        let profile_json = build_style_profile_json(text, existing.as_ref(), sample_count)?;
        self.write_style_profile(profile_json, sample_count)
    }

    pub fn style_profile(&self) -> anyhow::Result<Option<StyleProfileRecord>> {
        let conn = self.connection()?;
        conn.query_row(
            "SELECT id, profile_json, sample_count, updated_at
             FROM style_profile
             WHERE id = 'default'",
            [],
            |row| {
                Ok(StyleProfileRecord {
                    id: row.get(0)?,
                    profile_json: row.get(1)?,
                    sample_count: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn rebuild_style_profile_from_adopted_replies(
        &self,
    ) -> anyhow::Result<Option<StyleProfileRecord>> {
        let replies = self.adopted_reply_texts(STYLE_PROFILE_REBUILD_LIMIT)?;
        if replies.is_empty() {
            self.reset_style_profile()?;
            return Ok(None);
        }
        let sample_count = replies.len() as i64;
        let profile_json = build_style_profile_json_from_samples(&replies)?;
        self.write_style_profile(profile_json, sample_count)
            .map(Some)
    }

    pub fn reset_style_profile(&self) -> anyhow::Result<()> {
        let conn = self.connection()?;
        conn.execute("DELETE FROM style_profile WHERE id = 'default'", [])?;
        Ok(())
    }

    fn adopted_reply_texts(&self, limit: i64) -> anyhow::Result<Vec<String>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT text
             FROM messages
             WHERE role = 'me'
               AND approved = 1
               AND trim(text) <> ''
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |row| row.get::<_, String>(0))?;
        let mut replies = rows.collect::<Result<Vec<_>, _>>()?;
        replies.reverse();
        Ok(replies)
    }

    fn write_style_profile(
        &self,
        profile_json: String,
        sample_count: i64,
    ) -> anyhow::Result<StyleProfileRecord> {
        let conn = self.connection()?;
        let updated_at = now_rfc3339();
        conn.execute(
            "INSERT INTO style_profile (id, profile_json, sample_count, updated_at)
             VALUES ('default', ?1, ?2, ?3)
             ON CONFLICT(id) DO UPDATE SET
                profile_json = excluded.profile_json,
                sample_count = excluded.sample_count,
                updated_at = excluded.updated_at",
            params![&profile_json, sample_count, &updated_at],
        )?;
        Ok(StyleProfileRecord {
            id: "default".to_string(),
            profile_json,
            sample_count,
            updated_at,
        })
    }

    pub fn apply_retention(&self, retention_days: i64) -> anyhow::Result<()> {
        if retention_days <= 0 {
            return Ok(());
        }
        let cutoff = Utc::now() - Duration::days(retention_days);
        let cutoff = to_rfc3339(cutoff);
        let conn = self.connection()?;
        conn.execute(
            "DELETE FROM messages WHERE approved = 0 AND created_at < ?1",
            params![&cutoff],
        )?;
        conn.execute(
            "DELETE FROM context_summary WHERE created_at < ?1",
            params![&cutoff],
        )?;
        conn.execute(
            "DELETE FROM platform_signal_log WHERE created_at < ?1",
            params![&cutoff],
        )?;
        Ok(())
    }

    pub fn insert_context_summary(
        &self,
        summary: &ContextSummaryCandidate,
    ) -> anyhow::Result<ContextSummaryRecord> {
        self.insert_context_summary_for_contact(None, summary)
    }

    pub fn insert_context_summary_for_contact(
        &self,
        contact_id: Option<&str>,
        summary: &ContextSummaryCandidate,
    ) -> anyhow::Result<ContextSummaryRecord> {
        if summary.summary.trim().is_empty() {
            bail!("context summary is empty");
        }

        let record = ContextSummaryRecord {
            id: next_id("ctx"),
            contact_id: contact_id.unwrap_or_default().to_string(),
            source_kind: non_empty(&summary.source_kind, "clipboard"),
            source_ref: summary.source_ref.clone(),
            summary: summary.summary.clone(),
            created_at: now_rfc3339(),
        };

        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO context_summary (id, contact_id, source_kind, source_ref, summary, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                &record.id,
                &record.contact_id,
                &record.source_kind,
                &record.source_ref,
                &record.summary,
                &record.created_at
            ],
        )?;
        Ok(record)
    }

    pub fn delete_context_summary(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.connection()?;
        conn.execute("DELETE FROM context_summary WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn save_memory_candidate(
        &self,
        candidate: &MemoryCandidate,
    ) -> anyhow::Result<MemoryItemRecord> {
        self.save_memory_candidate_for_contact(None, candidate)
    }

    pub fn save_memory_candidate_for_contact(
        &self,
        contact_id: Option<&str>,
        candidate: &MemoryCandidate,
    ) -> anyhow::Result<MemoryItemRecord> {
        ensure_allowed_sensitivity(&candidate.sensitivity)?;
        if candidate.value.trim().is_empty() {
            bail!("memory value is empty");
        }
        let record = MemoryItemRecord {
            id: next_id("mem"),
            contact_id: contact_id.unwrap_or_default().to_string(),
            memory_type: non_empty(&candidate.memory_type, "event"),
            value: candidate.value.trim().to_string(),
            source_kind: non_empty(&candidate.source_kind, "clipboard"),
            source_ref: candidate.source_ref.clone(),
            source_excerpt: candidate.source_excerpt.clone(),
            confidence: candidate.confidence.clamp(0.0, 1.0),
            sensitivity: non_empty(&candidate.sensitivity, "normal"),
            expires_at: candidate.expires_at.clone(),
            status: "confirmed".to_string(),
            created_at: now_rfc3339(),
            updated_at: now_rfc3339(),
        };

        self.insert_memory_record(&record)?;
        Ok(record)
    }

    pub fn create_reminder_from_candidate(
        &self,
        candidate: &ReminderCandidate,
        trigger_at_override: Option<String>,
    ) -> anyhow::Result<ReminderDetail> {
        self.create_reminder_from_candidate_for_contact(None, candidate, trigger_at_override)
    }

    pub fn create_reminder_from_candidate_for_contact(
        &self,
        contact_id: Option<&str>,
        candidate: &ReminderCandidate,
        trigger_at_override: Option<String>,
    ) -> anyhow::Result<ReminderDetail> {
        ensure_allowed_sensitivity(&candidate.sensitivity)?;
        if candidate.memory_value.trim().is_empty() {
            bail!("reminder memory value is empty");
        }

        let memory = MemoryItemRecord {
            id: next_id("mem"),
            contact_id: contact_id.unwrap_or_default().to_string(),
            memory_type: non_empty(&candidate.memory_type, "event"),
            value: candidate.memory_value.trim().to_string(),
            source_kind: non_empty(&candidate.source_kind, "clipboard"),
            source_ref: candidate.source_ref.clone(),
            source_excerpt: candidate.source_excerpt.clone(),
            confidence: candidate.confidence.clamp(0.0, 1.0),
            sensitivity: non_empty(&candidate.sensitivity, "normal"),
            expires_at: String::new(),
            status: "confirmed".to_string(),
            created_at: now_rfc3339(),
            updated_at: now_rfc3339(),
        };
        self.insert_memory_record(&memory)?;

        let trigger_at = normalize_trigger_at(
            trigger_at_override
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(&candidate.trigger_at),
        )?;
        let reminder = ReminderRecord {
            id: next_id("rem"),
            memory_id: memory.id.clone(),
            trigger_at,
            reason: candidate.reason.clone(),
            suggested_follow_up: candidate.suggested_follow_up.clone(),
            status: "scheduled".to_string(),
            snooze_count: 0,
            created_at: now_rfc3339(),
            updated_at: now_rfc3339(),
        };

        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO reminder
                (id, memory_id, trigger_at, reason, suggested_follow_up, status, snooze_count, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                &reminder.id,
                &reminder.memory_id,
                &reminder.trigger_at,
                &reminder.reason,
                &reminder.suggested_follow_up,
                &reminder.status,
                reminder.snooze_count,
                &reminder.created_at,
                &reminder.updated_at
            ],
        )?;

        Ok(build_reminder_detail(reminder, memory))
    }

    pub fn due_reminders(&self, now: DateTime<Utc>) -> anyhow::Result<Vec<ReminderDetail>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT
                r.id, r.memory_id, r.trigger_at, r.reason, r.suggested_follow_up,
                r.status, r.snooze_count, r.created_at, r.updated_at,
                m.id, m.contact_id, m.type, m.value, m.source_kind, m.source_ref, m.source_excerpt,
                m.confidence, m.sensitivity, m.expires_at, m.status, m.created_at, m.updated_at
             FROM reminder r
             JOIN memory_item m ON m.id = r.memory_id
             WHERE r.status = 'scheduled' AND r.trigger_at <= ?1
             ORDER BY r.trigger_at ASC
             LIMIT 5",
        )?;
        let rows = stmt.query_map(params![to_rfc3339(now)], |row| {
            let reminder = ReminderRecord {
                id: row.get(0)?,
                memory_id: row.get(1)?,
                trigger_at: row.get(2)?,
                reason: row.get(3)?,
                suggested_follow_up: row.get(4)?,
                status: row.get(5)?,
                snooze_count: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            };
            let memory = memory_from_joined_row(row, 9)?;
            Ok(build_reminder_detail(reminder, memory))
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn latest_notified_reminder(&self) -> anyhow::Result<Option<ReminderDetail>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT
                r.id, r.memory_id, r.trigger_at, r.reason, r.suggested_follow_up,
                r.status, r.snooze_count, r.created_at, r.updated_at,
                m.id, m.contact_id, m.type, m.value, m.source_kind, m.source_ref, m.source_excerpt,
                m.confidence, m.sensitivity, m.expires_at, m.status, m.created_at, m.updated_at
             FROM reminder r
             JOIN memory_item m ON m.id = r.memory_id
             WHERE r.status = 'notified'
             ORDER BY r.updated_at DESC
             LIMIT 1",
        )?;
        stmt.query_row([], |row| {
            let reminder = ReminderRecord {
                id: row.get(0)?,
                memory_id: row.get(1)?,
                trigger_at: row.get(2)?,
                reason: row.get(3)?,
                suggested_follow_up: row.get(4)?,
                status: row.get(5)?,
                snooze_count: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            };
            let memory = memory_from_joined_row(row, 9)?;
            Ok(build_reminder_detail(reminder, memory))
        })
        .optional()
        .map_err(Into::into)
    }

    pub fn mark_reminder_notified(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "UPDATE reminder SET status = 'notified', updated_at = ?2 WHERE id = ?1",
            params![id, now_rfc3339()],
        )?;
        Ok(())
    }

    pub fn snooze_reminder(&self, id: &str, trigger_at: DateTime<Utc>) -> anyhow::Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "UPDATE reminder
             SET trigger_at = ?2, snooze_count = snooze_count + 1, updated_at = ?3
             WHERE id = ?1",
            params![id, to_rfc3339(trigger_at), now_rfc3339()],
        )?;
        Ok(())
    }

    pub fn delete_memory(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "UPDATE memory_item SET status = 'deleted', updated_at = ?2 WHERE id = ?1",
            params![id, now_rfc3339()],
        )?;
        conn.execute(
            "UPDATE reminder SET status = 'cancelled', updated_at = ?2 WHERE memory_id = ?1",
            params![id, now_rfc3339()],
        )?;
        Ok(())
    }

    pub fn delete_reminder(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "UPDATE reminder SET status = 'cancelled', updated_at = ?2 WHERE id = ?1",
            params![id, now_rfc3339()],
        )?;
        Ok(())
    }

    pub fn record_reply_feedback(
        &self,
        generation_id: &str,
        action: &str,
        candidate_index: i64,
    ) -> anyhow::Result<ReplyFeedbackRecord> {
        self.record_reply_feedback_for_contact(generation_id, action, candidate_index, "", None)
    }

    pub fn record_reply_feedback_for_contact(
        &self,
        generation_id: &str,
        action: &str,
        candidate_index: i64,
        candidate_text: &str,
        contact_id: Option<&str>,
    ) -> anyhow::Result<ReplyFeedbackRecord> {
        let record = ReplyFeedbackRecord {
            id: next_id("fb"),
            generation_id: generation_id.to_string(),
            action: action.to_string(),
            candidate_index,
            created_at: now_rfc3339(),
        };
        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO reply_feedback
                (id, generation_id, action, candidate_index, candidate_text, contact_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                &record.id,
                &record.generation_id,
                &record.action,
                record.candidate_index,
                candidate_text.chars().take(500).collect::<String>(),
                contact_id.unwrap_or_default(),
                &record.created_at
            ],
        )?;
        Ok(record)
    }

    pub fn has_recent_copy_feedback(&self, since: DateTime<Utc>) -> anyhow::Result<bool> {
        let conn = self.connection()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(1) FROM reply_feedback
             WHERE action = 'copy' AND created_at >= ?1",
            params![to_rfc3339(since)],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn insert_memory_record(&self, record: &MemoryItemRecord) -> anyhow::Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO memory_item
                (id, contact_id, type, value, source_kind, source_ref, source_excerpt, confidence, sensitivity, expires_at, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                &record.id,
                &record.contact_id,
                &record.memory_type,
                &record.value,
                &record.source_kind,
                &record.source_ref,
                &record.source_excerpt,
                record.confidence,
                &record.sensitivity,
                &record.expires_at,
                &record.status,
                &record.created_at,
                &record.updated_at
            ],
        )?;
        Ok(())
    }
}

fn build_reminder_detail(
    reminder: ReminderRecord,
    memory_item: MemoryItemRecord,
) -> ReminderDetail {
    let primary = if reminder.suggested_follow_up.trim().is_empty() {
        format!(
            "想起你之前说{}，现在怎么样啦？",
            short_value(&memory_item.value)
        )
    } else {
        reminder.suggested_follow_up.clone()
    };
    let value = short_value(&memory_item.value);
    ReminderDetail {
        reminder,
        memory_item,
        action_card: NextAction {
            action_type: "light_follow_up".to_string(),
            reason: "这条提醒来自你确认保存的聊天事件，适合低压关心一次，不需要追问细节。"
                .to_string(),
            confidence: 0.74,
        },
        follow_up_candidates: vec![
            Candidate {
                text: primary,
                style_tags: vec!["低压跟进".to_string()],
                risk_flags: vec!["none".to_string()],
                reason: "直接使用创建提醒时的跟进建议".to_string(),
            },
            Candidate {
                text: format!("刚想起你之前提到{}，还顺利吗？", value),
                style_tags: vec!["自然关心".to_string()],
                risk_flags: vec!["none".to_string()],
                reason: "轻轻提起来源，不逼对方展开".to_string(),
            },
            Candidate {
                text: format!("如果你愿意说的话，{}现在怎么样了？", value),
                style_tags: vec!["尊重边界".to_string()],
                risk_flags: vec!["none".to_string()],
                reason: "给对方不回应或少说的空间".to_string(),
            },
        ],
    }
}

fn default_db_path() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join("EchoMate").join("echomate.db");
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".echomate").join("echomate.db")
}

fn ensure_allowed_sensitivity(sensitivity: &str) -> anyhow::Result<()> {
    if sensitivity == "forbidden" {
        bail!("这条内容被标记为禁止保存，已按安全边界拦截");
    }
    Ok(())
}

fn normalize_trigger_at(value: &str) -> anyhow::Result<String> {
    if value.trim().is_empty() {
        return Ok(to_rfc3339(Utc::now() + Duration::hours(24)));
    }
    let parsed = DateTime::parse_from_rfc3339(value)
        .map_err(|e| anyhow!("提醒时间必须是 RFC3339 格式：{e}"))?;
    Ok(to_rfc3339(parsed.with_timezone(&Utc)))
}

fn non_empty(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

fn truncate_chars(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

fn now_rfc3339() -> String {
    to_rfc3339(Utc::now())
}

fn to_rfc3339(time: DateTime<Utc>) -> String {
    time.to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn next_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("{prefix}-{}-{nanos}", std::process::id())
}

fn short_value(value: &str) -> String {
    let trimmed = value.trim();
    let head = trimmed.chars().take(36).collect::<String>();
    if trimmed.chars().count() > 36 {
        format!("{head}...")
    } else {
        head
    }
}

fn contact_from_row(row: &Row<'_>) -> rusqlite::Result<ContactRecord> {
    let allowlisted: i64 = row.get(3)?;
    Ok(ContactRecord {
        id: row.get(0)?,
        alias: row.get(1)?,
        channel: row.get(2)?,
        is_allowlisted: allowlisted != 0,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn message_from_row(row: &Row<'_>) -> rusqlite::Result<MessageRecord> {
    let approved: i64 = row.get(5)?;
    Ok(MessageRecord {
        id: row.get(0)?,
        contact_id: row.get(1)?,
        role: row.get(2)?,
        text: row.get(3)?,
        source: row.get(4)?,
        approved: approved != 0,
        created_at: row.get(6)?,
    })
}

fn memory_from_row(row: &Row<'_>) -> rusqlite::Result<MemoryItemRecord> {
    memory_from_joined_row(row, 0)
}

fn memory_from_joined_row(row: &Row<'_>, start: usize) -> rusqlite::Result<MemoryItemRecord> {
    Ok(MemoryItemRecord {
        id: row.get(start)?,
        contact_id: row.get(start + 1)?,
        memory_type: row.get(start + 2)?,
        value: row.get(start + 3)?,
        source_kind: row.get(start + 4)?,
        source_ref: row.get(start + 5)?,
        source_excerpt: row.get(start + 6)?,
        confidence: row.get(start + 7)?,
        sensitivity: row.get(start + 8)?,
        expires_at: row.get(start + 9)?,
        status: row.get(start + 10)?,
        created_at: row.get(start + 11)?,
        updated_at: row.get(start + 12)?,
    })
}

fn bool_to_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

#[derive(Debug, Default, Clone)]
struct StyleSignals {
    sample_count: i64,
    total_chars: usize,
    question_count: i64,
    exclamation_count: i64,
    emoji_count: i64,
    short_count: i64,
    medium_count: i64,
    long_count: i64,
    latest_chars: usize,
    latest_tone: String,
}

impl StyleSignals {
    fn add_sample(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let chars = text.chars().count();
        self.sample_count += 1;
        self.total_chars += chars;
        if text.contains('？') || text.contains('?') {
            self.question_count += 1;
        }
        if text.contains('！') || text.contains('!') {
            self.exclamation_count += 1;
        }
        self.emoji_count += count_style_emoji_marks(text) as i64;
        if chars <= 22 {
            self.short_count += 1;
        } else if chars <= 45 {
            self.medium_count += 1;
        } else {
            self.long_count += 1;
        }
        self.latest_chars = chars;
        self.latest_tone = detect_style_tone(text).to_string();
    }

    fn avg_chars(&self) -> f64 {
        if self.sample_count <= 0 {
            0.0
        } else {
            self.total_chars as f64 / self.sample_count as f64
        }
    }

    fn question_rate(&self) -> f64 {
        ratio(self.question_count, self.sample_count)
    }

    fn exclamation_rate(&self) -> f64 {
        ratio(self.exclamation_count, self.sample_count)
    }

    fn emoji_avg(&self) -> f64 {
        ratio(self.emoji_count, self.sample_count)
    }

    fn short_ratio(&self) -> f64 {
        ratio(self.short_count, self.sample_count)
    }
}

fn build_style_profile_json(
    adopted_text: &str,
    existing: Option<&StyleProfileRecord>,
    sample_count: i64,
) -> anyhow::Result<String> {
    let previous_count = (sample_count - 1).max(0);
    let mut signals = existing
        .and_then(style_signals_from_profile)
        .unwrap_or_else(|| legacy_style_signals(existing, previous_count));
    signals.add_sample(adopted_text);
    build_style_profile_json_from_signals(&signals, "adopted_reply_summary")
}

fn build_style_profile_json_from_samples(samples: &[String]) -> anyhow::Result<String> {
    if samples.is_empty() {
        bail!("no adopted replies available for style profile");
    }
    let mut signals = StyleSignals::default();
    for sample in samples {
        signals.add_sample(sample);
    }
    build_style_profile_json_from_signals(&signals, "adopted_reply_rebuild")
}

fn build_style_profile_json_from_signals(
    signals: &StyleSignals,
    updated_from: &str,
) -> anyhow::Result<String> {
    if signals.sample_count <= 0 {
        bail!("no adopted replies available for style profile");
    }
    let avg_chars = signals.avg_chars();
    let sample_count = signals.sample_count;
    let length_label = length_profile_label(signals);
    let question_label = question_profile_label(signals.question_rate());
    let emoji_label = emoji_profile_label(signals.emoji_avg());
    let punctuation_label = punctuation_profile_label(signals.exclamation_rate());
    let tone_labels = style_tone_labels(signals);
    let generation_rules = style_generation_rules(signals);
    let avoid_rules = style_avoid_rules(signals);
    let prompt_guide = style_prompt_guide(&generation_rules, &avoid_rules);
    let summary = format!(
        "已采用 {sample_count} 条回复：平均约 {:.0} 字，{}；{}；{}；{}。这是可执行写作规则，不是完整人格画像。",
        avg_chars, length_label, question_label, emoji_label, punctuation_label
    );

    serde_json::to_string(&serde_json::json!({
        "summary": summary,
        "avg_chars": avg_chars,
        "tone_labels": tone_labels,
        "generation_rules": generation_rules,
        "avoid_rules": avoid_rules,
        "prompt_guide": prompt_guide,
        "signals": {
            "sample_count": sample_count,
            "total_chars": signals.total_chars,
            "question_count": signals.question_count,
            "question_rate": signals.question_rate(),
            "exclamation_count": signals.exclamation_count,
            "exclamation_rate": signals.exclamation_rate(),
            "emoji_count": signals.emoji_count,
            "emoji_avg": signals.emoji_avg(),
            "short_count": signals.short_count,
            "short_ratio": signals.short_ratio(),
            "medium_count": signals.medium_count,
            "long_count": signals.long_count,
            "latest_chars": signals.latest_chars,
            "latest_tone": signals.latest_tone,
        },
        "updated_from": updated_from
    }))
    .map_err(Into::into)
}

fn style_signals_from_profile(profile: &StyleProfileRecord) -> Option<StyleSignals> {
    let json = serde_json::from_str::<serde_json::Value>(&profile.profile_json).ok()?;
    let signals = json.get("signals")?;
    let sample_count = json_i64(signals, "sample_count").unwrap_or(profile.sample_count);
    Some(StyleSignals {
        sample_count,
        total_chars: json_i64(signals, "total_chars")
            .unwrap_or_else(|| {
                (json_f64(&json, "avg_chars").unwrap_or(0.0) * sample_count.max(0) as f64).round()
                    as i64
            })
            .max(0) as usize,
        question_count: json_i64(signals, "question_count").unwrap_or_default(),
        exclamation_count: json_i64(signals, "exclamation_count").unwrap_or_default(),
        emoji_count: json_i64(signals, "emoji_count").unwrap_or_default(),
        short_count: json_i64(signals, "short_count").unwrap_or_default(),
        medium_count: json_i64(signals, "medium_count").unwrap_or_default(),
        long_count: json_i64(signals, "long_count").unwrap_or_default(),
        latest_chars: json_i64(signals, "latest_chars").unwrap_or_default().max(0) as usize,
        latest_tone: json_string(signals, "latest_tone").unwrap_or_default(),
    })
}

fn legacy_style_signals(existing: Option<&StyleProfileRecord>, sample_count: i64) -> StyleSignals {
    let Some(profile) = existing else {
        return StyleSignals::default();
    };
    if sample_count <= 0 {
        return StyleSignals::default();
    }
    let json = serde_json::from_str::<serde_json::Value>(&profile.profile_json).ok();
    let avg_chars = json
        .as_ref()
        .and_then(|value| json_f64(value, "avg_chars"))
        .unwrap_or_default();
    let latest_tone = json
        .as_ref()
        .and_then(|value| {
            value
                .get("tone_labels")
                .and_then(|labels| labels.as_array())
                .and_then(|labels| labels.first())
                .and_then(|label| label.as_str())
                .map(ToString::to_string)
        })
        .unwrap_or_default();
    let mut signals = StyleSignals {
        sample_count,
        total_chars: (avg_chars * sample_count as f64).round() as usize,
        latest_chars: avg_chars.round().max(0.0) as usize,
        latest_tone: latest_tone.clone(),
        ..StyleSignals::default()
    };
    if avg_chars <= 22.0 {
        signals.short_count = sample_count;
    } else if avg_chars <= 45.0 {
        signals.medium_count = sample_count;
    } else {
        signals.long_count = sample_count;
    }
    if latest_tone.contains("提问") {
        signals.question_count = sample_count;
    }
    signals
}

fn style_tone_labels(signals: &StyleSignals) -> Vec<String> {
    let mut labels = vec![length_profile_label(signals).to_string()];
    if signals.question_rate() < 0.25 {
        labels.push("少追问".to_string());
    } else if signals.question_rate() >= 0.5 {
        labels.push("常用问句承接".to_string());
    }
    if signals.emoji_avg() < 0.2 {
        labels.push("少 emoji".to_string());
    }
    if !signals.latest_tone.is_empty() && !labels.iter().any(|label| label == &signals.latest_tone)
    {
        labels.push(signals.latest_tone.clone());
    }
    labels
}

fn style_generation_rules(signals: &StyleSignals) -> Vec<String> {
    let mut rules = Vec::new();
    let avg = signals.avg_chars();
    if avg <= 18.0 {
        rules.push("优先 8-20 字，一句话解决，不铺垫。".to_string());
    } else if avg <= 35.0 {
        rules.push("优先 12-35 字，短句为主，只表达一个重点。".to_string());
    } else {
        rules.push("可以 25-60 字，但要分清重点，避免长段解释。".to_string());
    }
    if signals.question_rate() < 0.25 {
        rules.push("默认先回应情绪或事实，不主动连续追问；需要推进时只问一个轻问题。".to_string());
    } else {
        rules.push("可以用短问句承接，但每条候选最多一个问题。".to_string());
    }
    if signals.emoji_avg() < 0.2 {
        rules.push("默认不用 emoji，标点保持克制。".to_string());
    } else {
        rules.push("emoji 可以少量使用，但不要堆叠。".to_string());
    }
    if signals.exclamation_rate() < 0.25 {
        rules.push("少用感叹号，语气保持低压自然。".to_string());
    }
    rules
}

fn style_avoid_rules(signals: &StyleSignals) -> Vec<String> {
    let mut rules = vec![
        "不要写成客服腔、鸡汤腔或总结报告。".to_string(),
        "不要替用户做承诺、邀约或强推进关系。".to_string(),
    ];
    if signals.short_ratio() >= 0.6 {
        rules.push("不要生成大段解释，避免把一句话扩成三句话。".to_string());
    }
    if signals.question_rate() < 0.25 {
        rules.push("不要为了显得热情而硬加问号。".to_string());
    }
    rules
}

fn style_prompt_guide(generation_rules: &[String], avoid_rules: &[String]) -> String {
    let mut parts = Vec::new();
    if !generation_rules.is_empty() {
        parts.push(format!("生成规则：{}", generation_rules.join("；")));
    }
    if !avoid_rules.is_empty() {
        parts.push(format!("避免：{}", avoid_rules.join("；")));
    }
    parts.join("。")
}

fn length_profile_label(signals: &StyleSignals) -> &'static str {
    if signals.avg_chars() <= 18.0 || signals.short_ratio() >= 0.6 {
        "短句低压"
    } else if signals.avg_chars() <= 35.0 {
        "短到中等"
    } else {
        "偏解释型"
    }
}

fn question_profile_label(rate: f64) -> &'static str {
    if rate < 0.25 {
        "很少追问"
    } else if rate < 0.5 {
        "偶尔用问句承接"
    } else {
        "经常用问句承接"
    }
}

fn emoji_profile_label(avg: f64) -> &'static str {
    if avg < 0.2 {
        "基本不用 emoji"
    } else if avg < 1.0 {
        "偶尔用 emoji"
    } else {
        "emoji 较多"
    }
}

fn punctuation_profile_label(exclamation_rate: f64) -> &'static str {
    if exclamation_rate < 0.25 {
        "感叹号很少"
    } else {
        "会用感叹号加强语气"
    }
}

fn detect_style_tone(text: &str) -> &'static str {
    if text.contains('？') || text.contains('?') {
        "接话提问"
    } else if text.chars().count() <= 22 {
        "简短低压"
    } else {
        "温和解释"
    }
}

fn count_style_emoji_marks(text: &str) -> usize {
    text.chars().filter(|ch| is_emoji_like(*ch)).count()
}

fn is_emoji_like(ch: char) -> bool {
    let code = ch as u32;
    (0x1F300..=0x1FAFF).contains(&code) || (0x2600..=0x27BF).contains(&code)
}

fn ratio(value: i64, total: i64) -> f64 {
    if total <= 0 {
        0.0
    } else {
        value as f64 / total as f64
    }
}

fn json_i64(value: &serde_json::Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|entry| entry.as_i64())
}

fn json_f64(value: &serde_json::Value, key: &str) -> Option<f64> {
    value.get(key).and_then(|entry| entry.as_f64())
}

fn json_string(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|entry| entry.as_str())
        .map(ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_repo_saves_memory_and_due_reminder() {
        let path = std::env::temp_dir().join(format!("echomate-test-{}.db", next_id("repo")));
        let repo = MemoryRepository::new(path.clone()).expect("repo");

        let memory = repo
            .save_memory_candidate(&MemoryCandidate {
                memory_type: "event".to_string(),
                value: "她明天面试".to_string(),
                source_kind: "clipboard".to_string(),
                source_ref: "clipboard".to_string(),
                source_excerpt: "我明天面试".to_string(),
                confidence: 0.88,
                sensitivity: "normal".to_string(),
                expires_at: String::new(),
            })
            .expect("save memory");
        assert_eq!(memory.status, "confirmed");

        let detail = repo
            .create_reminder_from_candidate(
                &ReminderCandidate {
                    memory_type: "event".to_string(),
                    memory_value: "她明天面试".to_string(),
                    source_kind: "clipboard".to_string(),
                    source_ref: "clipboard".to_string(),
                    source_excerpt: "我明天面试".to_string(),
                    recommended_time: "今晚".to_string(),
                    trigger_at: to_rfc3339(Utc::now() - Duration::seconds(1)),
                    reason: "面试后适合轻问结果".to_string(),
                    suggested_follow_up: "今天面试还顺利吗？".to_string(),
                    confidence: 0.8,
                    sensitivity: "normal".to_string(),
                },
                None,
            )
            .expect("create reminder");
        assert_eq!(detail.follow_up_candidates.len(), 3);

        let due = repo.due_reminders(Utc::now()).expect("due reminders");
        assert_eq!(due.len(), 1);

        repo.mark_reminder_notified(&due[0].reminder.id)
            .expect("mark notified");
        assert!(repo
            .due_reminders(Utc::now())
            .expect("due again")
            .is_empty());

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn memory_repo_rejects_forbidden_memory() {
        let path = std::env::temp_dir().join(format!("echomate-test-{}.db", next_id("repo")));
        let repo = MemoryRepository::new(path.clone()).expect("repo");
        let err = repo
            .save_memory_candidate(&MemoryCandidate {
                memory_type: "event".to_string(),
                value: "不该保存".to_string(),
                source_kind: "clipboard".to_string(),
                source_ref: String::new(),
                source_excerpt: "secret".to_string(),
                confidence: 0.7,
                sensitivity: "forbidden".to_string(),
                expires_at: String::new(),
            })
            .expect_err("forbidden should fail");
        assert!(err.to_string().contains("禁止保存"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn contacts_messages_retention_and_style_profile_work() {
        let path = std::env::temp_dir().join(format!("echomate-test-{}.db", next_id("repo")));
        let repo = MemoryRepository::new(path.clone()).expect("repo");

        let contact = repo
            .upsert_contact(&ContactInput {
                id: None,
                alias: "齐齐".to_string(),
                channel: "wechat".to_string(),
                is_allowlisted: true,
            })
            .expect("upsert contact");
        assert!(contact.is_allowlisted);
        assert!(repo
            .find_allowlisted_contact("齐齐", "wechat")
            .expect("find contact")
            .is_some());

        let inbound = repo
            .append_message(
                &contact.id,
                "other",
                "我明天面试，有点紧张",
                "notification",
                false,
            )
            .expect("append inbound");
        assert_eq!(inbound.source, "notification");
        repo.record_platform_signal_log(
            &contact.id,
            &contact.alias,
            &contact.channel,
            "notification",
            "WeChat",
            "我明天面试，有点紧张",
            true,
            "白名单联系人有新的近似入站信号。",
        )
        .expect("signal log");
        assert_eq!(
            repo.platform_signal_log_count(&contact.id)
                .expect("signal count"),
            1
        );
        let adopted = repo
            .append_message(
                &contact.id,
                "me",
                "明天面试顺利，别给自己太大压力。",
                "manual",
                true,
            )
            .expect("append adopted");
        assert!(adopted.approved);

        let recent = repo.recent_messages(&contact.id, 10).expect("recent");
        assert_eq!(recent.len(), 2);

        let saved = repo
            .save_memory_candidate_for_contact(
                Some(&contact.id),
                &MemoryCandidate {
                    memory_type: "event".to_string(),
                    value: "她明天有面试".to_string(),
                    source_kind: "notification".to_string(),
                    source_ref: "toast".to_string(),
                    source_excerpt: "我明天面试".to_string(),
                    confidence: 0.9,
                    sensitivity: "normal".to_string(),
                    expires_at: String::new(),
                },
            )
            .expect("save scoped memory");
        assert_eq!(saved.contact_id, contact.id);
        assert_eq!(
            repo.confirmed_memories_for_contact(&contact.id, 5)
                .expect("memories")
                .len(),
            1
        );

        let profile = repo
            .update_style_profile_from_reply("明天面试顺利，别给自己太大压力。")
            .expect("style profile");
        assert_eq!(profile.sample_count, 1);
        assert!(profile.profile_json.contains("summary"));
        let rebuilt = repo
            .rebuild_style_profile_from_adopted_replies()
            .expect("rebuild style profile")
            .expect("rebuilt profile");
        assert_eq!(rebuilt.sample_count, 1);
        assert!(rebuilt.profile_json.contains("adopted_reply_rebuild"));
        repo.reset_style_profile().expect("reset style profile");
        assert!(repo.style_profile().expect("profile after reset").is_none());

        repo.apply_retention(30).expect("retention");
        repo.clear_contact_context(&contact.id)
            .expect("clear contact");
        assert!(repo
            .recent_messages(&contact.id, 10)
            .expect("recent after clear")
            .is_empty());
        assert_eq!(
            repo.confirmed_memories_for_contact(&contact.id, 5)
                .expect("memories after clear")
                .len(),
            0
        );
        assert_eq!(
            repo.platform_signal_log_count(&contact.id)
                .expect("signal count after clear"),
            0
        );
        assert!(repo
            .rebuild_style_profile_from_adopted_replies()
            .expect("empty style rebuild")
            .is_none());

        let _ = std::fs::remove_file(path);
    }
}
