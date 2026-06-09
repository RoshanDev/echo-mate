// Memory repository for style profiles, contact facts, and reminder MVP data.
use crate::domain::{
    Candidate, ContactFactCandidate, ContactFactRecord, ContactInput, ContactRecord,
    ContextSummaryCandidate, ContextSummaryRecord, DataAuditCount, DataAuditReport,
    DataContaminationFinding, EditedMemoryCandidate, MemoryCandidate, MemoryCandidateRecord,
    MemoryItemRecord, MemoryUsageSummary, MessageRecord, NextAction, RelationshipCard,
    ReminderCandidate, ReminderCenterItem, ReminderDetail, ReminderRecord, ReplyFeedbackRecord,
    ScreenshotAnalysis, SourceCard, SourceContextRecord, StyleProfileRecord, SuggestionRunRecord,
};
use crate::store::migrations::run_migrations;
use anyhow::{anyhow, bail};
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::collections::HashSet;
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
            "DELETE FROM message_events WHERE contact_id = ?1",
            params![id],
        )?;
        conn.execute(
            "DELETE FROM memory_candidates WHERE contact_id = ?1",
            params![id],
        )?;
        conn.execute(
            "DELETE FROM memory_usage_log WHERE contact_id = ?1",
            params![id],
        )?;
        conn.execute(
            "DELETE FROM suggestion_runs WHERE contact_id = ?1",
            params![id],
        )?;
        conn.execute(
            "DELETE FROM contact_facts WHERE contact_id = ?1",
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
        conn.execute(
            "DELETE FROM source_contexts WHERE contact_id = ?1",
            params![id],
        )?;
        conn.execute("DELETE FROM contacts WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn clear_contact_context(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.connection()?;
        conn.execute("DELETE FROM messages WHERE contact_id = ?1", params![id])?;
        conn.execute(
            "DELETE FROM message_events WHERE contact_id = ?1",
            params![id],
        )?;
        conn.execute(
            "DELETE FROM memory_candidates WHERE contact_id = ?1",
            params![id],
        )?;
        conn.execute(
            "DELETE FROM memory_usage_log WHERE contact_id = ?1",
            params![id],
        )?;
        conn.execute(
            "DELETE FROM reply_feedback WHERE contact_id = ?1",
            params![id],
        )?;
        conn.execute(
            "DELETE FROM suggestion_runs WHERE contact_id = ?1",
            params![id],
        )?;
        conn.execute(
            "DELETE FROM contact_facts WHERE contact_id = ?1",
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
        conn.execute(
            "DELETE FROM source_contexts WHERE contact_id = ?1",
            params![id],
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
        self.append_message_with_source_context(
            contact_id, role, text, source, approved, None, None, None, None, 0.0,
        )
    }

    pub fn append_message_with_source_context(
        &self,
        contact_id: &str,
        role: &str,
        text: &str,
        source: &str,
        approved: bool,
        source_context_id: Option<&str>,
        captured_at: Option<&str>,
        visible_message_time: Option<&str>,
        inferred_chat_time: Option<&str>,
        source_confidence: f64,
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
            "INSERT INTO messages
                (id, contact_id, role, text, source, approved, created_at,
                 source_context_id, captured_at, visible_message_time, inferred_chat_time, source_confidence)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                &record.id,
                &record.contact_id,
                &record.role,
                &record.text,
                &record.source,
                bool_to_i64(record.approved),
                &record.created_at,
                source_context_id.unwrap_or_default(),
                captured_at.unwrap_or_default(),
                visible_message_time.unwrap_or_default(),
                inferred_chat_time.unwrap_or_default(),
                source_confidence.clamp(0.0, 1.0)
            ],
        )?;
        let source_context_id_value = source_context_id.unwrap_or_default();
        let source_meta = if source_context_id_value.trim().is_empty() {
            None
        } else {
            source_context_event_meta(&conn, source_context_id_value)?
        };
        let input_kind = source_meta
            .as_ref()
            .map(|meta| meta.input_kind.as_str())
            .unwrap_or(source);
        let fact_source = source_meta
            .as_ref()
            .map(|meta| meta.fact_source.as_str())
            .unwrap_or(source);
        let provider = source_meta
            .as_ref()
            .map(|meta| meta.provider.as_str())
            .unwrap_or_default();
        let captured_at_value = source_meta
            .as_ref()
            .map(|meta| meta.captured_at.as_str())
            .or(captured_at)
            .unwrap_or_default();
        let visible_message_time_value = source_meta
            .as_ref()
            .map(|meta| meta.visible_message_time.as_str())
            .or(visible_message_time)
            .unwrap_or_default();
        let inferred_chat_time_value = source_meta
            .as_ref()
            .map(|meta| meta.inferred_chat_time.as_str())
            .or(inferred_chat_time)
            .unwrap_or_default();
        let source_confidence_value = source_meta
            .as_ref()
            .map(|meta| meta.source_confidence)
            .unwrap_or_else(|| source_confidence.clamp(0.0, 1.0));
        conn.execute(
            "INSERT INTO message_events
                (id, message_id, contact_id, role, text_excerpt, provider, input_kind, fact_source,
                 source_context_id, created_at, captured_at, visible_message_time, inferred_chat_time,
                 source_confidence)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                next_id("mev"),
                &record.id,
                &record.contact_id,
                &record.role,
                truncate_chars(&record.text, 500),
                provider,
                non_empty(input_kind, "manual"),
                non_empty(fact_source, "manual"),
                source_context_id_value,
                &record.created_at,
                captured_at_value,
                visible_message_time_value,
                inferred_chat_time_value,
                source_confidence_value
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

    pub fn insert_source_context(
        &self,
        contact_id: &str,
        provider: &str,
        input_kind: &str,
        fact_source: &str,
        source_label: &str,
        source_excerpt: &str,
        captured_at: Option<&str>,
        visible_message_time: Option<&str>,
        inferred_chat_time: Option<&str>,
        source_confidence: f64,
        metadata_json: &str,
    ) -> anyhow::Result<SourceContextRecord> {
        if contact_id.trim().is_empty() {
            bail!("contact_id is required to save a source context");
        }
        let now = now_rfc3339();
        let record = SourceContextRecord {
            id: next_id("src"),
            contact_id: contact_id.to_string(),
            provider: provider.trim().to_string(),
            input_kind: non_empty(input_kind, "clipboard"),
            fact_source: non_empty(fact_source, input_kind),
            source_label: source_label.trim().to_string(),
            source_excerpt: truncate_chars(source_excerpt.trim(), 500),
            created_at: now.clone(),
            captured_at: captured_at.unwrap_or(&now).to_string(),
            visible_message_time: visible_message_time.unwrap_or_default().to_string(),
            inferred_chat_time: inferred_chat_time.unwrap_or_default().to_string(),
            source_confidence: source_confidence.clamp(0.0, 1.0),
            metadata_json: if metadata_json.trim().is_empty() {
                "{}".to_string()
            } else {
                metadata_json.to_string()
            },
        };

        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO source_contexts
                (id, contact_id, provider, input_kind, fact_source, source_label, source_excerpt,
                 created_at, captured_at, visible_message_time, inferred_chat_time,
                 source_confidence, metadata_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                &record.id,
                &record.contact_id,
                &record.provider,
                &record.input_kind,
                &record.fact_source,
                &record.source_label,
                &record.source_excerpt,
                &record.created_at,
                &record.captured_at,
                &record.visible_message_time,
                &record.inferred_chat_time,
                record.source_confidence,
                &record.metadata_json
            ],
        )?;
        Ok(record)
    }

    pub fn record_suggestion_run(
        &self,
        contact_id: &str,
        provider: &str,
        input_kind: &str,
        source_context_id: Option<&str>,
        source_cards: &[SourceCard],
        output_summary: &str,
    ) -> anyhow::Result<SuggestionRunRecord> {
        if contact_id.trim().is_empty() {
            bail!("contact_id is required to save a suggestion run");
        }
        let conn = self.connection()?;
        let source_context_id_value = source_context_id.unwrap_or_default();
        let source_meta = if source_context_id_value.trim().is_empty() {
            None
        } else {
            source_context_event_meta(&conn, source_context_id_value)?
        };
        let created_at = now_rfc3339();
        let input_kind_value = non_empty(input_kind, "clipboard");
        let fact_source = source_meta
            .as_ref()
            .map(|meta| meta.fact_source.clone())
            .unwrap_or_else(|| input_kind_value.clone());
        let captured_at = source_meta
            .as_ref()
            .map(|meta| fallback_if_empty(meta.captured_at.clone(), created_at.clone()))
            .unwrap_or_else(|| created_at.clone());
        let record = SuggestionRunRecord {
            id: next_id("run"),
            contact_id: contact_id.to_string(),
            provider: provider.trim().to_string(),
            input_kind: input_kind_value,
            fact_source,
            source_context_id: source_context_id_value.to_string(),
            context_sources_json: serde_json::to_string(source_cards)?,
            output_summary: truncate_chars(output_summary.trim(), 500),
            created_at,
            captured_at,
            visible_message_time: source_meta
                .as_ref()
                .map(|meta| meta.visible_message_time.clone())
                .unwrap_or_default(),
            inferred_chat_time: source_meta
                .as_ref()
                .map(|meta| meta.inferred_chat_time.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            source_confidence: source_meta
                .as_ref()
                .map(|meta| meta.source_confidence)
                .unwrap_or_default(),
        };
        conn.execute(
            "INSERT INTO suggestion_runs
                (id, contact_id, provider, input_kind, fact_source, source_context_id,
                 context_sources_json, output_summary, created_at, captured_at,
                 visible_message_time, inferred_chat_time, source_confidence)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                &record.id,
                &record.contact_id,
                &record.provider,
                &record.input_kind,
                &record.fact_source,
                &record.source_context_id,
                &record.context_sources_json,
                &record.output_summary,
                &record.created_at,
                &record.captured_at,
                &record.visible_message_time,
                &record.inferred_chat_time,
                record.source_confidence
            ],
        )?;
        Ok(record)
    }

    pub fn insert_screenshot_analysis(
        &self,
        contact_id: &str,
        source_context_id: Option<&str>,
        image_path: &str,
        image_width: u32,
        image_height: u32,
        ocr_provider: &str,
        analysis: &ScreenshotAnalysis,
    ) -> anyhow::Result<()> {
        let confidence = if analysis.turns.is_empty() {
            0.0
        } else {
            analysis
                .turns
                .iter()
                .map(|turn| turn.confidence)
                .sum::<f64>()
                / analysis.turns.len() as f64
        };
        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO screenshot_analyses
                (id, contact_id, source_context_id, image_path, image_width, image_height,
                 parser_version, ocr_provider, turns_json, last_reply_target,
                 visible_time_label, inferred_chat_time, staleness, warnings_json,
                 confidence, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'screenshot-v2-local-ocr', ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                next_id("shot"),
                contact_id,
                source_context_id.unwrap_or_default(),
                image_path,
                image_width as i64,
                image_height as i64,
                ocr_provider,
                serde_json::to_string(&analysis.turns)?,
                &analysis.last_reply_target,
                &analysis.visible_time_label,
                &analysis.inferred_chat_time,
                &analysis.staleness,
                serde_json::to_string(&analysis.warnings)?,
                confidence.clamp(0.0, 1.0),
                now_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn record_memory_candidates_for_run(
        &self,
        contact_id: &str,
        suggestion_run_id: &str,
        source_context_id: Option<&str>,
        candidates: &[MemoryCandidate],
    ) -> anyhow::Result<usize> {
        if contact_id.trim().is_empty() {
            bail!("contact_id is required to save memory candidates");
        }
        if candidates.is_empty() {
            return Ok(0);
        }
        let conn = self.connection()?;
        let source_context_id_value = source_context_id.unwrap_or_default();
        let source_meta = if source_context_id_value.trim().is_empty() {
            None
        } else {
            source_context_event_meta(&conn, source_context_id_value)?
        };
        let mut inserted = 0;
        for (index, candidate) in candidates.iter().enumerate() {
            if candidate.value.trim().is_empty() {
                continue;
            }
            let created_at = now_rfc3339();
            let input_kind = source_meta
                .as_ref()
                .map(|meta| meta.input_kind.as_str())
                .unwrap_or(&candidate.source_kind);
            let fact_source = source_meta
                .as_ref()
                .map(|meta| meta.fact_source.as_str())
                .unwrap_or(&candidate.source_kind);
            conn.execute(
                "INSERT INTO memory_candidates
                    (id, contact_id, suggestion_run_id, source_context_id, candidate_index,
                     memory_type, summary, value, source_kind, source_ref, source_excerpt,
                     source_quote, reason, fact_source, confidence, sensitivity, expires_at,
                     ttl_days, status, created_at, captured_at, visible_message_time,
                     inferred_chat_time, source_confidence)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, 'candidate', ?19, ?20, ?21, ?22, ?23)",
                params![
                    next_id("mcand"),
                    contact_id,
                    suggestion_run_id,
                    source_context_id_value,
                    index as i64,
                    non_empty(&candidate.memory_type, "event"),
                    truncate_chars(
                        non_empty(&candidate.summary, candidate.value.trim()).trim(),
                        500
                    ),
                    truncate_chars(candidate.value.trim(), 500),
                    non_empty(&candidate.source_kind, input_kind),
                    truncate_chars(candidate.source_ref.trim(), 200),
                    truncate_chars(candidate.source_excerpt.trim(), 500),
                    truncate_chars(
                        non_empty(&candidate.source_quote, &candidate.source_excerpt).trim(),
                        500
                    ),
                    truncate_chars(candidate.reason.trim(), 500),
                    non_empty(fact_source, "unknown"),
                    candidate.confidence.clamp(0.0, 1.0),
                    non_empty(&candidate.sensitivity, "normal"),
                    &candidate.expires_at,
                    candidate.ttl_days,
                    &created_at,
                    source_meta
                        .as_ref()
                        .map(|meta| meta.captured_at.as_str())
                        .unwrap_or_default(),
                    source_meta
                        .as_ref()
                        .map(|meta| meta.visible_message_time.as_str())
                        .unwrap_or_default(),
                    source_meta
                        .as_ref()
                        .map(|meta| meta.inferred_chat_time.as_str())
                        .unwrap_or_default(),
                    source_meta
                        .as_ref()
                        .map(|meta| meta.source_confidence)
                        .unwrap_or_default()
                ],
            )?;
            inserted += 1;
        }
        Ok(inserted)
    }

    pub fn record_candidate_memory_usage(
        &self,
        contact_id: &str,
        suggestion_run_id: &str,
        candidates: &[Candidate],
    ) -> anyhow::Result<usize> {
        if contact_id.trim().is_empty()
            || suggestion_run_id.trim().is_empty()
            || candidates.is_empty()
        {
            return Ok(0);
        }
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id
             FROM memory_item
             WHERE contact_id = ?1 AND status = 'confirmed'",
        )?;
        let rows = stmt.query_map(params![contact_id], |row| row.get::<_, String>(0))?;
        let memory_ids = rows.collect::<Result<HashSet<_>, _>>()?;
        if memory_ids.is_empty() {
            return Ok(0);
        }

        let mut inserted = 0;
        for (candidate_index, candidate) in candidates.iter().enumerate() {
            let mut seen_for_candidate = HashSet::new();
            for source_ref in &candidate.source_refs {
                let memory_id = memory_id_from_source_ref(source_ref);
                if memory_id.is_empty()
                    || !memory_ids.contains(&memory_id)
                    || !seen_for_candidate.insert(memory_id.clone())
                {
                    continue;
                }
                let created_at = now_rfc3339();
                conn.execute(
                    "INSERT INTO memory_usage_log
                        (id, contact_id, memory_id, suggestion_run_id, candidate_index,
                         candidate_text, source_ref, usage_reason, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        next_id("muse"),
                        contact_id,
                        &memory_id,
                        suggestion_run_id,
                        candidate_index as i64,
                        truncate_chars(candidate.text.trim(), 220),
                        truncate_chars(source_ref.trim(), 160),
                        truncate_chars(candidate.reason.trim(), 300),
                        &created_at
                    ],
                )?;
                conn.execute(
                    "UPDATE memory_item SET last_used_at = ?2 WHERE id = ?1",
                    params![&memory_id, &created_at],
                )?;
                inserted += 1;
            }
        }
        Ok(inserted)
    }

    pub fn recent_source_cards(
        &self,
        contact_id: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<SourceCard>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, input_kind, fact_source, source_label, source_excerpt,
                    created_at, captured_at, visible_message_time, inferred_chat_time, source_confidence
             FROM source_contexts
             WHERE contact_id = ?1
             ORDER BY created_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![contact_id, limit as i64], |row| {
            let id: String = row.get(0)?;
            let source_kind: String = row.get(1)?;
            let fact_source: String = row.get(2)?;
            let source_label: String = row.get(3)?;
            let source_excerpt: String = row.get(4)?;
            let created_at: String = row.get(5)?;
            let captured_at: String = row.get(6)?;
            Ok(SourceCard {
                id,
                source_kind,
                title: source_label,
                detail: source_excerpt,
                fact_source,
                captured_at: fallback_if_empty(captured_at, created_at),
                visible_message_time: row.get(7)?,
                inferred_chat_time: row.get(8)?,
                source_confidence: row.get(9)?,
            })
        })?;
        let mut cards = rows.collect::<Result<Vec<_>, _>>()?;
        cards.reverse();
        Ok(cards)
    }

    pub fn list_contact_facts(&self, contact_id: &str) -> anyhow::Result<Vec<ContactFactRecord>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, contact_id, fact_type, value, normalized_value, source_note,
                    provider, input_kind, fact_source, sensitivity, confidence, ttl_days,
                    usage_policy, created_at, captured_at, visible_message_time,
                    inferred_chat_time, source_confidence, updated_at, last_used_at
             FROM contact_facts
             WHERE contact_id = ?1
             ORDER BY updated_at DESC",
        )?;
        let rows = stmt.query_map(params![contact_id], contact_fact_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn prompt_contact_facts(
        &self,
        contact_id: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<ContactFactRecord>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT id, contact_id, fact_type, value, normalized_value, source_note,
                    provider, input_kind, fact_source, sensitivity, confidence, ttl_days,
                    usage_policy, created_at, captured_at, visible_message_time,
                    inferred_chat_time, source_confidence, updated_at, last_used_at
             FROM contact_facts
             WHERE contact_id = ?1
               AND sensitivity NOT IN ('high', 'forbidden')
               AND usage_policy NOT IN ('never', 'disabled')
             ORDER BY updated_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![contact_id, limit as i64], contact_fact_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn save_contact_facts(
        &self,
        contact_id: &str,
        facts: &[ContactFactCandidate],
    ) -> anyhow::Result<Vec<ContactFactRecord>> {
        if contact_id.trim().is_empty() {
            bail!("contact_id is required to save contact facts");
        }
        let conn = self.connection()?;
        let mut saved = Vec::new();
        for fact in facts {
            ensure_allowed_sensitivity(&fact.sensitivity)?;
            let value = fact.value.trim();
            if value.is_empty() {
                continue;
            }
            let fact_type = non_empty(&fact.fact_type, "note");
            let normalized_value = fallback_if_empty(
                fact.normalized_value.trim().to_lowercase(),
                value.trim().to_lowercase(),
            );
            let fact_source = non_empty(&fact.fact_source, "manual");
            let now = now_rfc3339();
            let id = next_id("fact");
            conn.execute(
                "INSERT INTO contact_facts
                    (id, contact_id, fact_type, value, normalized_value, source_note,
                     provider, input_kind, fact_source, sensitivity, confidence, ttl_days,
                     usage_policy, created_at, captured_at, visible_message_time,
                     inferred_chat_time, source_confidence, updated_at, last_used_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, '', 'manual', ?7, ?8, ?9, ?10, ?11, ?12, ?12, '', 'unknown', ?9, ?13, '')
                 ON CONFLICT(contact_id, fact_type, normalized_value, fact_source) DO UPDATE SET
                    value = excluded.value,
                    source_note = excluded.source_note,
                    provider = excluded.provider,
                    input_kind = excluded.input_kind,
                    sensitivity = excluded.sensitivity,
                    confidence = excluded.confidence,
                    ttl_days = excluded.ttl_days,
                    usage_policy = excluded.usage_policy,
                    captured_at = excluded.captured_at,
                    visible_message_time = excluded.visible_message_time,
                    inferred_chat_time = excluded.inferred_chat_time,
                    source_confidence = excluded.source_confidence,
                    updated_at = excluded.updated_at",
                params![
                    &id,
                    contact_id,
                    &fact_type,
                    value,
                    &normalized_value,
                    truncate_chars(fact.source_note.trim(), 500),
                    &fact_source,
                    non_empty(&fact.sensitivity, "normal"),
                    fact.confidence.clamp(0.0, 1.0),
                    fact.ttl_days,
                    non_empty(&fact.usage_policy, "contextual"),
                    &now,
                    &now,
                ],
            )?;
            let record = self.get_contact_fact_by_key(
                contact_id,
                &fact_type,
                &normalized_value,
                &fact_source,
            )?;
            saved.push(record);
        }
        Ok(saved)
    }

    pub fn delete_contact_fact(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.connection()?;
        conn.execute("DELETE FROM contact_facts WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn contact_fact_count(&self, contact_id: &str) -> anyhow::Result<i64> {
        let conn = self.connection()?;
        conn.query_row(
            "SELECT COUNT(*) FROM contact_facts WHERE contact_id = ?1",
            params![contact_id],
            |row| row.get(0),
        )
        .map_err(Into::into)
    }

    pub fn suggestion_run_count(&self, contact_id: &str) -> anyhow::Result<i64> {
        let conn = self.connection()?;
        conn.query_row(
            "SELECT COUNT(*) FROM suggestion_runs WHERE contact_id = ?1",
            params![contact_id],
            |row| row.get(0),
        )
        .map_err(Into::into)
    }

    pub fn message_event_count(&self, contact_id: &str) -> anyhow::Result<i64> {
        let conn = self.connection()?;
        conn.query_row(
            "SELECT COUNT(*) FROM message_events WHERE contact_id = ?1",
            params![contact_id],
            |row| row.get(0),
        )
        .map_err(Into::into)
    }

    pub fn memory_candidate_count(&self, contact_id: &str) -> anyhow::Result<i64> {
        let conn = self.connection()?;
        conn.query_row(
            "SELECT COUNT(*) FROM memory_candidates WHERE contact_id = ?1",
            params![contact_id],
            |row| row.get(0),
        )
        .map_err(Into::into)
    }

    pub fn list_memory_candidates(
        &self,
        contact_id: &str,
        status: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<Vec<MemoryCandidateRecord>> {
        let conn = self.connection()?;
        let status = status.unwrap_or("candidate");
        let mut stmt = conn.prepare(
            "SELECT id, contact_id, suggestion_run_id, source_context_id, candidate_index,
                    memory_type, summary, value, source_kind, source_ref, source_excerpt,
                    source_quote, reason, fact_source, confidence, sensitivity, expires_at,
                    ttl_days, status, created_at, captured_at, visible_message_time,
                    inferred_chat_time, source_confidence
             FROM memory_candidates
             WHERE contact_id = ?1 AND status = ?2
             ORDER BY created_at DESC
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(
            params![contact_id, status, limit as i64],
            memory_candidate_from_row,
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn confirm_memory_candidate(&self, id: &str) -> anyhow::Result<MemoryItemRecord> {
        self.confirm_memory_candidate_with_edits(&EditedMemoryCandidate {
            id: id.to_string(),
            memory_type: String::new(),
            value: String::new(),
            source_excerpt: String::new(),
            sensitivity: String::new(),
            ttl_days: None,
            clear_ttl: false,
        })
    }

    pub fn confirm_memory_candidate_with_edits(
        &self,
        edited: &EditedMemoryCandidate,
    ) -> anyhow::Result<MemoryItemRecord> {
        let conn = self.connection()?;
        let candidate = conn.query_row(
            "SELECT id, contact_id, suggestion_run_id, source_context_id, candidate_index,
                    memory_type, summary, value, source_kind, source_ref, source_excerpt,
                    source_quote, reason, fact_source, confidence, sensitivity, expires_at,
                    ttl_days, status, created_at, captured_at, visible_message_time,
                    inferred_chat_time, source_confidence
             FROM memory_candidates
             WHERE id = ?1
             LIMIT 1",
            params![&edited.id],
            memory_candidate_from_row,
        )?;
        if candidate.status != "candidate" {
            bail!("这条候选记忆已处理，不能重复保存");
        }
        let ttl_days = if edited.clear_ttl {
            None
        } else {
            edited.ttl_days.or(candidate.ttl_days)
        };
        let expires_at = if edited.clear_ttl {
            String::new()
        } else if let Some(days) = ttl_days {
            to_rfc3339(Utc::now() + Duration::days(days.max(1)))
        } else if candidate.expires_at.trim().is_empty() {
            String::new()
        } else {
            candidate.expires_at.clone()
        };
        let value = non_empty(&edited.value, &candidate.value);
        if value.trim().is_empty() {
            bail!("候选记忆内容为空，不能保存");
        }
        let saved = self.save_memory_candidate_for_contact(
            Some(&candidate.contact_id),
            &MemoryCandidate {
                memory_type: non_empty(&edited.memory_type, &candidate.memory_type),
                summary: candidate.summary.clone(),
                value,
                source_kind: candidate.source_kind.clone(),
                source_ref: candidate.source_ref.clone(),
                source_excerpt: fallback_if_empty(
                    edited.source_excerpt.clone(),
                    fallback_if_empty(
                        candidate.source_excerpt.clone(),
                        candidate.source_quote.clone(),
                    ),
                ),
                source_quote: candidate.source_quote.clone(),
                reason: candidate.reason.clone(),
                confidence: candidate.confidence,
                sensitivity: non_empty(&edited.sensitivity, &candidate.sensitivity),
                expires_at,
                ttl_days,
            },
        )?;
        let conn = self.connection()?;
        conn.execute(
            "UPDATE memory_candidates
             SET status = 'confirmed',
                 memory_type = ?2,
                 value = ?3,
                 source_excerpt = ?4,
                 sensitivity = ?5,
                 expires_at = ?6,
                 ttl_days = ?7
             WHERE id = ?1",
            params![
                &edited.id,
                &saved.memory_type,
                &saved.value,
                &saved.source_excerpt,
                &saved.sensitivity,
                &saved.expires_at,
                ttl_days
            ],
        )?;
        Ok(saved)
    }

    pub fn ignore_memory_candidate_record(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "UPDATE memory_candidates SET status = 'ignored' WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn list_reminders(
        &self,
        contact_id: Option<&str>,
        include_cancelled: bool,
        limit: usize,
    ) -> anyhow::Result<Vec<ReminderCenterItem>> {
        let conn = self.connection()?;
        let mut sql = String::from(
            "SELECT
                r.id, r.memory_id, r.contact_id, r.kind, r.due_at, r.trigger_at,
                r.reason, r.suggested_follow_up, r.source_memory_id, r.source_context_id,
                r.cooldown_key, r.status, r.snooze_until, r.snooze_count, r.created_at, r.updated_at,
                m.id, m.contact_id, m.type, m.value, m.source_kind, m.source_ref, m.source_excerpt,
                m.confidence, m.sensitivity, m.expires_at, m.status, m.created_at, m.updated_at, m.last_used_at
             FROM reminder r
             JOIN memory_item m ON m.id = r.memory_id",
        );
        let mut filters = Vec::new();
        if contact_id.filter(|id| !id.trim().is_empty()).is_some() {
            filters.push("r.contact_id = ?1");
        }
        if !include_cancelled {
            filters.push("r.status NOT IN ('cancelled', 'deleted')");
        }
        if !filters.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&filters.join(" AND "));
        }
        sql.push_str(" ORDER BY r.trigger_at ASC LIMIT ?");
        let limit_index = if contact_id.filter(|id| !id.trim().is_empty()).is_some() {
            2
        } else {
            1
        };
        sql.push_str(&limit_index.to_string());

        let mut stmt = conn.prepare(&sql)?;
        let map_row = |row: &Row<'_>| {
            Ok(ReminderCenterItem {
                reminder: reminder_from_row(row, 0)?,
                memory_item: memory_from_joined_row(row, 16)?,
            })
        };
        let rows = if let Some(contact_id) = contact_id.filter(|id| !id.trim().is_empty()) {
            stmt.query_map(params![contact_id, limit as i64], map_row)?
        } else {
            stmt.query_map(params![limit as i64], map_row)?
        };
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn complete_reminder(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.connection()?;
        conn.execute(
            "UPDATE reminder SET status = 'completed', updated_at = ?2 WHERE id = ?1",
            params![id, now_rfc3339()],
        )?;
        Ok(())
    }

    pub fn data_audit_report(
        &self,
        active_contact_id: &str,
        retention_days: i64,
    ) -> anyhow::Result<DataAuditReport> {
        let conn = self.connection()?;
        let table_names = [
            "contacts",
            "messages",
            "message_events",
            "memory_item",
            "memory_usage_log",
            "memory_candidates",
            "reminder",
            "reminder_mutes",
            "context_summary",
            "source_contexts",
            "screenshot_analyses",
            "suggestion_runs",
            "contact_facts",
            "reply_feedback",
            "platform_signal_log",
            "style_profile",
        ];
        let mut counts = Vec::new();
        for table_name in table_names {
            counts.push(DataAuditCount {
                table_name: table_name.to_string(),
                count: table_count(&conn, table_name)?,
            });
        }
        Ok(DataAuditReport {
            generated_at: now_rfc3339(),
            active_contact_id: active_contact_id.to_string(),
            counts,
            contamination_findings: self.scan_for_test_artifacts()?,
            retention_days,
        })
    }

    pub fn export_data_snapshot(&self) -> anyhow::Result<serde_json::Value> {
        let conn = self.connection()?;
        Ok(serde_json::json!({
            "exported_at": now_rfc3339(),
            "contacts": query_json_rows(&conn, "SELECT id, alias, channel, is_allowlisted, created_at, updated_at FROM contacts ORDER BY updated_at DESC")?,
            "contact_facts": query_json_rows(&conn, "SELECT id, contact_id, fact_type, value, normalized_value, source_note, fact_source, sensitivity, confidence, ttl_days, usage_policy, created_at, updated_at, last_used_at FROM contact_facts ORDER BY updated_at DESC")?,
            "memories": query_json_rows(&conn, "SELECT id, contact_id, type, value, source_kind, source_ref, source_excerpt, confidence, sensitivity, expires_at, status, created_at, updated_at, last_used_at FROM memory_item ORDER BY updated_at DESC")?,
            "memory_usage_log": query_json_rows(&conn, "SELECT id, contact_id, memory_id, suggestion_run_id, candidate_index, candidate_text, source_ref, usage_reason, created_at FROM memory_usage_log ORDER BY created_at DESC")?,
            "reminders": query_json_rows(&conn, "SELECT id, memory_id, contact_id, kind, due_at, trigger_at, reason, suggested_follow_up, source_context_id, cooldown_key, status, snooze_until, snooze_count, created_at, updated_at FROM reminder ORDER BY updated_at DESC")?,
            "context_summaries": query_json_rows(&conn, "SELECT id, contact_id, source_kind, source_ref, summary, created_at FROM context_summary ORDER BY created_at DESC")?,
            "source_contexts": query_json_rows(&conn, "SELECT id, contact_id, provider, input_kind, fact_source, source_label, source_excerpt, created_at, captured_at, visible_message_time, inferred_chat_time, source_confidence FROM source_contexts ORDER BY created_at DESC")?,
            "suggestion_runs": query_json_rows(&conn, "SELECT id, contact_id, provider, input_kind, fact_source, source_context_id, output_summary, created_at, captured_at, visible_message_time, inferred_chat_time, source_confidence FROM suggestion_runs ORDER BY created_at DESC")?,
        }))
    }

    pub fn clear_all_data(&self) -> anyhow::Result<()> {
        let conn = self.connection()?;
        for table in [
            "reply_feedback",
            "platform_signal_log",
            "reminder_mutes",
            "reminder",
            "memory_usage_log",
            "memory_candidates",
            "memory_item",
            "message_events",
            "messages",
            "context_summary",
            "screenshot_analyses",
            "suggestion_runs",
            "source_contexts",
            "contact_facts",
            "contacts",
            "style_profile",
        ] {
            conn.execute(&format!("DELETE FROM {table}"), [])?;
        }
        Ok(())
    }

    pub fn mute_reminders(
        &self,
        contact_id: Option<&str>,
        kind: Option<&str>,
        hours: i64,
        reason: &str,
    ) -> anyhow::Result<()> {
        let muted_until = to_rfc3339(Utc::now() + Duration::hours(hours.clamp(1, 24 * 365)));
        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO reminder_mutes (id, contact_id, kind, muted_until, reason, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                next_id("mute"),
                contact_id.unwrap_or_default(),
                kind.unwrap_or_default(),
                muted_until,
                truncate_chars(reason, 200),
                now_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn reminder_is_muted(
        &self,
        reminder: &ReminderRecord,
        now: DateTime<Utc>,
    ) -> anyhow::Result<bool> {
        let conn = self.connection()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(1)
             FROM reminder_mutes
             WHERE muted_until > ?1
               AND (contact_id = '' OR contact_id = ?2)
               AND (kind = '' OR kind = ?3)",
            params![to_rfc3339(now), &reminder.contact_id, &reminder.kind],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn recent_notified_reminder_count(
        &self,
        contact_id: &str,
        since: DateTime<Utc>,
    ) -> anyhow::Result<i64> {
        let conn = self.connection()?;
        conn.query_row(
            "SELECT COUNT(1)
             FROM reminder
             WHERE contact_id = ?1
               AND status IN ('notified', 'completed')
               AND updated_at >= ?2",
            params![contact_id, to_rfc3339(since)],
            |row| row.get(0),
        )
        .map_err(Into::into)
    }

    pub fn relationship_card(&self, contact_id: &str) -> anyhow::Result<RelationshipCard> {
        let contact = self
            .get_contact(contact_id)?
            .ok_or_else(|| anyhow!("contact not found"))?;
        let recent_messages = self.recent_messages(contact_id, 8)?;
        let contact_facts = self.list_contact_facts(contact_id)?;
        let memories = self.confirmed_memories_for_contact(contact_id, 8)?;
        let memory_ids = memories
            .iter()
            .map(|memory| memory.id.clone())
            .collect::<Vec<_>>();
        let memory_usages = self.memory_usage_summaries(&memory_ids)?;
        let pending_memory_candidates =
            self.list_memory_candidates(contact_id, Some("candidate"), 8)?;
        let reminders = self.list_reminders(Some(contact_id), false, 8)?;
        let style_profile = self.style_profile()?;
        let last_stop = recent_messages
            .last()
            .map(|message| {
                format!(
                    "{}：{}",
                    message_capture_label_for_audit(message),
                    truncate_chars(&message.text, 80)
                )
            })
            .unwrap_or_else(|| "还没有本地上下文。".to_string());
        let boundary_notes = contact_facts
            .iter()
            .filter(|fact| matches!(fact.fact_type.as_str(), "boundary"))
            .map(|fact| fact.value.clone())
            .collect::<Vec<_>>();
        Ok(RelationshipCard {
            contact,
            recent_messages,
            contact_facts,
            memories,
            memory_usages,
            pending_memory_candidates,
            reminders,
            style_profile,
            interaction_cadence: "根据最近保存/采纳记录估计；信息不足时保持低压，不主动追问。"
                .to_string(),
            last_stop,
            boundary_notes,
        })
    }

    pub fn scan_for_test_artifacts(&self) -> anyhow::Result<Vec<DataContaminationFinding>> {
        let conn = self.connection()?;
        let mut findings = Vec::new();
        scan_table_for_test_artifacts(
            &conn,
            &mut findings,
            "contacts",
            "id",
            "id",
            &["alias", "channel"],
        )?;
        scan_table_for_test_artifacts(
            &conn,
            &mut findings,
            "messages",
            "id",
            "contact_id",
            &["role", "text", "source", "source_context_id"],
        )?;
        scan_table_for_test_artifacts(
            &conn,
            &mut findings,
            "message_events",
            "id",
            "contact_id",
            &[
                "message_id",
                "role",
                "text_excerpt",
                "provider",
                "input_kind",
                "fact_source",
                "source_context_id",
            ],
        )?;
        scan_table_for_test_artifacts(
            &conn,
            &mut findings,
            "memory_item",
            "id",
            "contact_id",
            &[
                "type",
                "value",
                "source_kind",
                "source_ref",
                "source_excerpt",
                "fact_source",
                "source_context_id",
                "status",
            ],
        )?;
        scan_table_for_test_artifacts(
            &conn,
            &mut findings,
            "memory_usage_log",
            "id",
            "contact_id",
            &[
                "memory_id",
                "suggestion_run_id",
                "candidate_text",
                "source_ref",
                "usage_reason",
            ],
        )?;
        scan_table_for_test_artifacts(
            &conn,
            &mut findings,
            "memory_candidates",
            "id",
            "contact_id",
            &[
                "suggestion_run_id",
                "source_context_id",
                "memory_type",
                "value",
                "source_kind",
                "source_ref",
                "source_excerpt",
                "fact_source",
                "status",
            ],
        )?;
        scan_table_for_test_artifacts(
            &conn,
            &mut findings,
            "context_summary",
            "id",
            "contact_id",
            &["source_kind", "source_ref", "summary", "source_context_id"],
        )?;
        scan_table_for_test_artifacts(
            &conn,
            &mut findings,
            "platform_signal_log",
            "id",
            "contact_id",
            &[
                "contact_alias",
                "channel",
                "source",
                "app_name",
                "text_excerpt",
                "reason",
            ],
        )?;
        scan_table_for_test_artifacts(
            &conn,
            &mut findings,
            "source_contexts",
            "id",
            "contact_id",
            &[
                "provider",
                "input_kind",
                "fact_source",
                "source_label",
                "source_excerpt",
                "metadata_json",
            ],
        )?;
        scan_table_for_test_artifacts(
            &conn,
            &mut findings,
            "suggestion_runs",
            "id",
            "contact_id",
            &[
                "provider",
                "input_kind",
                "fact_source",
                "source_context_id",
                "context_sources_json",
                "output_summary",
            ],
        )?;
        scan_table_for_test_artifacts(
            &conn,
            &mut findings,
            "reply_feedback",
            "id",
            "contact_id",
            &[
                "generation_id",
                "suggestion_run_id",
                "action",
                "candidate_text",
            ],
        )?;
        scan_table_for_test_artifacts(
            &conn,
            &mut findings,
            "contact_facts",
            "id",
            "contact_id",
            &[
                "fact_type",
                "value",
                "normalized_value",
                "source_note",
                "provider",
                "input_kind",
                "fact_source",
                "usage_policy",
                "source_context_id",
            ],
        )?;
        Ok(findings)
    }

    fn get_contact_fact_by_key(
        &self,
        contact_id: &str,
        fact_type: &str,
        normalized_value: &str,
        fact_source: &str,
    ) -> anyhow::Result<ContactFactRecord> {
        let conn = self.connection()?;
        conn.query_row(
            "SELECT id, contact_id, fact_type, value, normalized_value, source_note,
                    provider, input_kind, fact_source, sensitivity, confidence, ttl_days,
                    usage_policy, created_at, captured_at, visible_message_time,
                    inferred_chat_time, source_confidence, updated_at, last_used_at
             FROM contact_facts
             WHERE contact_id = ?1 AND fact_type = ?2 AND normalized_value = ?3 AND fact_source = ?4
             LIMIT 1",
            params![contact_id, fact_type, normalized_value, fact_source],
            contact_fact_from_row,
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
                confidence, sensitivity, expires_at, status, created_at, updated_at, last_used_at
             FROM memory_item
             WHERE contact_id = ?1 AND status = 'confirmed'
             ORDER BY updated_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![contact_id, limit as i64], memory_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn memory_usage_summaries(
        &self,
        memory_ids: &[String],
    ) -> anyhow::Result<Vec<MemoryUsageSummary>> {
        if memory_ids.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.connection()?;
        let mut summaries = Vec::new();
        for memory_id in memory_ids {
            let (usage_count, last_used_at): (i64, Option<String>) = conn.query_row(
                "SELECT COUNT(1), MAX(created_at)
                 FROM memory_usage_log
                 WHERE memory_id = ?1",
                params![memory_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            let mut ref_stmt = conn.prepare(
                "SELECT candidate_text
                 FROM memory_usage_log
                 WHERE memory_id = ?1
                 ORDER BY created_at DESC
                 LIMIT 3",
            )?;
            let rows = ref_stmt.query_map(params![memory_id], |row| row.get::<_, String>(0))?;
            summaries.push(MemoryUsageSummary {
                memory_id: memory_id.clone(),
                usage_count,
                last_used_at: last_used_at.unwrap_or_default(),
                recent_references: rows.collect::<Result<Vec<_>, _>>()?,
            });
        }
        Ok(summaries)
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
        self.insert_context_summary_with_source(contact_id, summary, None, None, None, None, 0.0)
    }

    pub fn insert_context_summary_with_source(
        &self,
        contact_id: Option<&str>,
        summary: &ContextSummaryCandidate,
        source_context_id: Option<&str>,
        captured_at: Option<&str>,
        visible_message_time: Option<&str>,
        inferred_chat_time: Option<&str>,
        source_confidence: f64,
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
            "INSERT INTO context_summary
                (id, contact_id, source_kind, source_ref, summary, created_at,
                 source_context_id, captured_at, visible_message_time, inferred_chat_time, source_confidence)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                &record.id,
                &record.contact_id,
                &record.source_kind,
                &record.source_ref,
                &record.summary,
                &record.created_at,
                source_context_id.unwrap_or_default(),
                captured_at.unwrap_or_default(),
                visible_message_time.unwrap_or_default(),
                inferred_chat_time.unwrap_or_default(),
                source_confidence.clamp(0.0, 1.0)
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
            last_used_at: String::new(),
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
            last_used_at: String::new(),
        };
        self.insert_memory_record(&memory)?;

        let trigger_at = normalize_trigger_at(
            trigger_at_override
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(&candidate.trigger_at),
        )?;
        let kind = non_empty(&candidate.kind, "follow_up");
        let cooldown_key = non_empty(
            &candidate.cooldown_key,
            &format!(
                "{}:{}",
                memory.contact_id,
                non_empty(&candidate.memory_type, "event")
            ),
        );
        let reminder = ReminderRecord {
            id: next_id("rem"),
            memory_id: memory.id.clone(),
            contact_id: memory.contact_id.clone(),
            kind,
            due_at: trigger_at.clone(),
            trigger_at,
            reason: candidate.reason.clone(),
            suggested_follow_up: candidate.suggested_follow_up.clone(),
            source_memory_id: memory.id.clone(),
            source_context_id: candidate.source_context_id.clone(),
            cooldown_key,
            status: "scheduled".to_string(),
            snooze_until: String::new(),
            snooze_count: 0,
            created_at: now_rfc3339(),
            updated_at: now_rfc3339(),
        };

        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO reminder
                (id, memory_id, contact_id, kind, due_at, trigger_at, reason, suggested_follow_up,
                 source_memory_id, source_context_id, cooldown_key, status, snooze_until,
                 snooze_count, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                &reminder.id,
                &reminder.memory_id,
                &reminder.contact_id,
                &reminder.kind,
                &reminder.due_at,
                &reminder.trigger_at,
                &reminder.reason,
                &reminder.suggested_follow_up,
                &reminder.source_memory_id,
                &reminder.source_context_id,
                &reminder.cooldown_key,
                &reminder.status,
                &reminder.snooze_until,
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
                r.id, r.memory_id, r.contact_id, r.kind, r.due_at, r.trigger_at,
                r.reason, r.suggested_follow_up, r.source_memory_id, r.source_context_id,
                r.cooldown_key, r.status, r.snooze_until, r.snooze_count, r.created_at, r.updated_at,
                m.id, m.contact_id, m.type, m.value, m.source_kind, m.source_ref, m.source_excerpt,
                m.confidence, m.sensitivity, m.expires_at, m.status, m.created_at, m.updated_at, m.last_used_at
             FROM reminder r
             JOIN memory_item m ON m.id = r.memory_id
             WHERE r.status = 'scheduled' AND r.trigger_at <= ?1
             ORDER BY r.trigger_at ASC
             LIMIT 5",
        )?;
        let rows = stmt.query_map(params![to_rfc3339(now)], |row| {
            let reminder = reminder_from_row(row, 0)?;
            let memory = memory_from_joined_row(row, 16)?;
            Ok(build_reminder_detail(reminder, memory))
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn latest_notified_reminder(&self) -> anyhow::Result<Option<ReminderDetail>> {
        let conn = self.connection()?;
        let mut stmt = conn.prepare(
            "SELECT
                r.id, r.memory_id, r.contact_id, r.kind, r.due_at, r.trigger_at,
                r.reason, r.suggested_follow_up, r.source_memory_id, r.source_context_id,
                r.cooldown_key, r.status, r.snooze_until, r.snooze_count, r.created_at, r.updated_at,
                m.id, m.contact_id, m.type, m.value, m.source_kind, m.source_ref, m.source_excerpt,
                m.confidence, m.sensitivity, m.expires_at, m.status, m.created_at, m.updated_at, m.last_used_at
             FROM reminder r
             JOIN memory_item m ON m.id = r.memory_id
             WHERE r.status = 'notified'
             ORDER BY r.updated_at DESC
             LIMIT 1",
        )?;
        stmt.query_row([], |row| {
            let reminder = reminder_from_row(row, 0)?;
            let memory = memory_from_joined_row(row, 16)?;
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
             SET trigger_at = ?2, due_at = ?2, snooze_until = ?2,
                 status = 'scheduled', snooze_count = snooze_count + 1, updated_at = ?3
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
                (id, contact_id, type, value, source_kind, source_ref, source_excerpt, confidence, sensitivity, expires_at, status, created_at, updated_at, last_used_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
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
                &record.updated_at,
                &record.last_used_at
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
    let reminder_id = reminder.id.clone();
    let memory_id = memory_item.id.clone();
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
                intent_group: "支持".to_string(),
                style_tags: vec!["低压跟进".to_string()],
                risk_flags: vec!["none".to_string()],
                source_refs: vec![reminder_id.clone(), memory_id.clone()],
                reason: "直接使用创建提醒时的跟进建议".to_string(),
            },
            Candidate {
                text: format!("刚想起你之前提到{}，还顺利吗？", value),
                intent_group: "温柔".to_string(),
                style_tags: vec!["自然关心".to_string()],
                risk_flags: vec!["none".to_string()],
                source_refs: vec![reminder_id.clone(), memory_id.clone()],
                reason: "轻轻提起来源，不逼对方展开".to_string(),
            },
            Candidate {
                text: format!("如果你愿意说的话，{}现在怎么样了？", value),
                intent_group: "收束".to_string(),
                style_tags: vec!["尊重边界".to_string()],
                risk_flags: vec!["none".to_string()],
                source_refs: vec![reminder_id, memory_id],
                reason: "给对方不回应或少说的空间".to_string(),
            },
        ],
    }
}

fn default_db_path() -> PathBuf {
    if e2e_mock_provider_enabled() {
        return e2e_profile_dir().join("echomate.db");
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        return PathBuf::from(appdata).join("EchoMate").join("echomate.db");
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".echomate").join("echomate.db")
}

fn e2e_mock_provider_enabled() -> bool {
    std::env::var("ECHOMATE_E2E_MOCK_PROVIDER")
        .map(|value| value == "1")
        .unwrap_or(false)
}

fn e2e_profile_dir() -> PathBuf {
    std::env::var("ECHOMATE_E2E_PROFILE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::temp_dir().join(format!("echomate-e2e-profile-{}", std::process::id()))
        })
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
        last_used_at: row.get(start + 13)?,
    })
}

fn reminder_from_row(row: &Row<'_>, start: usize) -> rusqlite::Result<ReminderRecord> {
    Ok(ReminderRecord {
        id: row.get(start)?,
        memory_id: row.get(start + 1)?,
        contact_id: row.get(start + 2)?,
        kind: row.get(start + 3)?,
        due_at: row.get(start + 4)?,
        trigger_at: row.get(start + 5)?,
        reason: row.get(start + 6)?,
        suggested_follow_up: row.get(start + 7)?,
        source_memory_id: row.get(start + 8)?,
        source_context_id: row.get(start + 9)?,
        cooldown_key: row.get(start + 10)?,
        status: row.get(start + 11)?,
        snooze_until: row.get(start + 12)?,
        snooze_count: row.get(start + 13)?,
        created_at: row.get(start + 14)?,
        updated_at: row.get(start + 15)?,
    })
}

struct SourceContextEventMeta {
    provider: String,
    input_kind: String,
    fact_source: String,
    captured_at: String,
    visible_message_time: String,
    inferred_chat_time: String,
    source_confidence: f64,
}

fn source_context_event_meta(
    conn: &Connection,
    source_context_id: &str,
) -> anyhow::Result<Option<SourceContextEventMeta>> {
    conn.query_row(
        "SELECT provider, input_kind, fact_source, captured_at, visible_message_time,
                inferred_chat_time, source_confidence
         FROM source_contexts
         WHERE id = ?1
         LIMIT 1",
        params![source_context_id],
        |row| {
            Ok(SourceContextEventMeta {
                provider: row.get(0)?,
                input_kind: row.get(1)?,
                fact_source: row.get(2)?,
                captured_at: row.get(3)?,
                visible_message_time: row.get(4)?,
                inferred_chat_time: row.get(5)?,
                source_confidence: row.get(6)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

const TEST_ARTIFACT_MARKERS: &[&str] = &[
    "e2e",
    "e2e-mock",
    "mock",
    "test contact",
    "test_contact",
    "测试联系人",
];

fn scan_table_for_test_artifacts(
    conn: &Connection,
    findings: &mut Vec<DataContaminationFinding>,
    table_name: &str,
    id_column: &str,
    contact_column: &str,
    fields: &[&str],
) -> anyhow::Result<()> {
    let selected_fields = fields.join(", ");
    let sql = format!("SELECT {id_column}, {contact_column}, {selected_fields} FROM {table_name}",);
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        let record_id: String = row.get(0)?;
        let contact_id: String = row.get(1)?;
        let mut values = Vec::with_capacity(fields.len());
        for (offset, field) in fields.iter().enumerate() {
            let value: String = row.get(offset + 2)?;
            values.push(((*field).to_string(), value));
        }
        Ok((record_id, contact_id, values))
    })?;

    for row in rows {
        let (record_id, contact_id, values) = row?;
        for (field_name, value) in values {
            if let Some(marker) = test_artifact_marker(&value) {
                findings.push(DataContaminationFinding {
                    table_name: table_name.to_string(),
                    record_id: record_id.clone(),
                    contact_id: contact_id.clone(),
                    field_name,
                    matched_text: marker.to_string(),
                    reason: "检测到 e2e/mock/test 标记，不能出现在真实联系人上下文。".to_string(),
                });
            }
        }
    }
    Ok(())
}

fn test_artifact_marker(value: &str) -> Option<&'static str> {
    let lower = value.to_lowercase();
    TEST_ARTIFACT_MARKERS
        .iter()
        .copied()
        .find(|marker| lower.contains(&marker.to_lowercase()))
}

fn memory_candidate_from_row(row: &Row<'_>) -> rusqlite::Result<MemoryCandidateRecord> {
    Ok(MemoryCandidateRecord {
        id: row.get(0)?,
        contact_id: row.get(1)?,
        suggestion_run_id: row.get(2)?,
        source_context_id: row.get(3)?,
        candidate_index: row.get(4)?,
        memory_type: row.get(5)?,
        summary: row.get(6)?,
        value: row.get(7)?,
        source_kind: row.get(8)?,
        source_ref: row.get(9)?,
        source_excerpt: row.get(10)?,
        source_quote: row.get(11)?,
        reason: row.get(12)?,
        fact_source: row.get(13)?,
        confidence: row.get(14)?,
        sensitivity: row.get(15)?,
        expires_at: row.get(16)?,
        ttl_days: row.get(17)?,
        status: row.get(18)?,
        created_at: row.get(19)?,
        captured_at: row.get(20)?,
        visible_message_time: row.get(21)?,
        inferred_chat_time: row.get(22)?,
        source_confidence: row.get(23)?,
    })
}

fn table_count(conn: &Connection, table_name: &str) -> anyhow::Result<i64> {
    conn.query_row(&format!("SELECT COUNT(*) FROM {table_name}"), [], |row| {
        row.get(0)
    })
    .map_err(Into::into)
}

fn query_json_rows(conn: &Connection, sql: &str) -> anyhow::Result<Vec<serde_json::Value>> {
    let mut stmt = conn.prepare(sql)?;
    let columns = stmt
        .column_names()
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let rows = stmt.query_map([], |row| {
        let mut object = serde_json::Map::new();
        for (index, name) in columns.iter().enumerate() {
            let value = match row.get_ref(index)? {
                rusqlite::types::ValueRef::Null => serde_json::Value::Null,
                rusqlite::types::ValueRef::Integer(value) => serde_json::json!(value),
                rusqlite::types::ValueRef::Real(value) => serde_json::json!(value),
                rusqlite::types::ValueRef::Text(value) => {
                    serde_json::json!(String::from_utf8_lossy(value).to_string())
                }
                rusqlite::types::ValueRef::Blob(value) => {
                    serde_json::json!(format!("<{} bytes>", value.len()))
                }
            };
            object.insert(name.clone(), value);
        }
        Ok(serde_json::Value::Object(object))
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn message_capture_label_for_audit(message: &MessageRecord) -> String {
    format!("{} / {}", message.source, message.created_at)
}

fn contact_fact_from_row(row: &Row<'_>) -> rusqlite::Result<ContactFactRecord> {
    Ok(ContactFactRecord {
        id: row.get(0)?,
        contact_id: row.get(1)?,
        fact_type: row.get(2)?,
        value: row.get(3)?,
        normalized_value: row.get(4)?,
        source_note: row.get(5)?,
        provider: row.get(6)?,
        input_kind: row.get(7)?,
        fact_source: row.get(8)?,
        sensitivity: row.get(9)?,
        confidence: row.get(10)?,
        ttl_days: row.get(11)?,
        usage_policy: row.get(12)?,
        created_at: row.get(13)?,
        captured_at: row.get(14)?,
        visible_message_time: row.get(15)?,
        inferred_chat_time: row.get(16)?,
        source_confidence: row.get(17)?,
        updated_at: row.get(18)?,
        last_used_at: row.get(19)?,
    })
}

fn bool_to_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

fn fallback_if_empty(value: String, fallback: String) -> String {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

fn memory_id_from_source_ref(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let after_prefix = trimmed
        .strip_prefix("memory:")
        .or_else(|| trimmed.strip_prefix("mem:"))
        .unwrap_or(trimmed);
    after_prefix
        .split(|ch: char| ch.is_whitespace() || matches!(ch, '/' | ',' | ';' | '，' | '；'))
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
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
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn memory_repo_saves_memory_and_due_reminder() {
        let path = std::env::temp_dir().join(format!("echomate-test-{}.db", next_id("repo")));
        let repo = MemoryRepository::new(path.clone()).expect("repo");

        let memory = repo
            .save_memory_candidate(&MemoryCandidate {
                memory_type: "event".to_string(),
                summary: "她明天面试".to_string(),
                value: "她明天面试".to_string(),
                source_kind: "clipboard".to_string(),
                source_ref: "clipboard".to_string(),
                source_excerpt: "我明天面试".to_string(),
                source_quote: "我明天面试".to_string(),
                reason: "明确事件".to_string(),
                confidence: 0.88,
                sensitivity: "normal".to_string(),
                expires_at: String::new(),
                ttl_days: None,
            })
            .expect("save memory");
        assert_eq!(memory.status, "confirmed");

        let detail = repo
            .create_reminder_from_candidate(
                &ReminderCandidate {
                    kind: "follow_up".to_string(),
                    memory_type: "event".to_string(),
                    memory_value: "她明天面试".to_string(),
                    source_kind: "clipboard".to_string(),
                    source_ref: "clipboard".to_string(),
                    source_excerpt: "我明天面试".to_string(),
                    recommended_time: "今晚".to_string(),
                    trigger_at: to_rfc3339(Utc::now() - Duration::seconds(1)),
                    reason: "面试后适合轻问结果".to_string(),
                    suggested_follow_up: "今天面试还顺利吗？".to_string(),
                    source_context_id: String::new(),
                    cooldown_key: "event:interview".to_string(),
                    confidence: 0.8,
                    sensitivity: "normal".to_string(),
                },
                None,
            )
            .expect("create reminder");
        assert_eq!(detail.follow_up_candidates.len(), 3);
        assert_eq!(detail.reminder.kind, "follow_up");
        assert_eq!(detail.reminder.due_at, detail.reminder.trigger_at);
        let reminders = repo
            .list_reminders(None, false, 10)
            .expect("list reminders");
        assert_eq!(reminders.len(), 1);
        assert_eq!(reminders[0].reminder.status, "scheduled");

        let due = repo.due_reminders(Utc::now()).expect("due reminders");
        assert_eq!(due.len(), 1);

        repo.mark_reminder_notified(&due[0].reminder.id)
            .expect("mark notified");
        assert!(repo
            .due_reminders(Utc::now())
            .expect("due again")
            .is_empty());
        repo.complete_reminder(&detail.reminder.id)
            .expect("complete reminder");
        let reminders = repo
            .list_reminders(None, false, 10)
            .expect("list reminders after complete");
        assert_eq!(reminders[0].reminder.status, "completed");
        let audit = repo.data_audit_report("", 30).expect("data audit report");
        assert!(audit
            .counts
            .iter()
            .any(|item| item.table_name == "reminder" && item.count == 1));
        let exported = repo.export_data_snapshot().expect("export data");
        assert!(exported
            .get("reminders")
            .and_then(|value| value.as_array())
            .is_some());
        repo.clear_all_data().expect("clear all data");
        let audit_after_clear = repo
            .data_audit_report("", 30)
            .expect("data audit after clear");
        assert!(audit_after_clear.counts.iter().all(|item| item.count == 0));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn memory_repo_rejects_forbidden_memory() {
        let path = std::env::temp_dir().join(format!("echomate-test-{}.db", next_id("repo")));
        let repo = MemoryRepository::new(path.clone()).expect("repo");
        let err = repo
            .save_memory_candidate(&MemoryCandidate {
                memory_type: "event".to_string(),
                summary: "不该保存".to_string(),
                value: "不该保存".to_string(),
                source_kind: "clipboard".to_string(),
                source_ref: String::new(),
                source_excerpt: "secret".to_string(),
                source_quote: "secret".to_string(),
                reason: "敏感测试".to_string(),
                confidence: 0.7,
                sensitivity: "forbidden".to_string(),
                expires_at: String::new(),
                ttl_days: None,
            })
            .expect_err("forbidden should fail");
        assert!(err.to_string().contains("禁止保存"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn memory_candidate_confirm_with_edits_sets_ttl() {
        let path = std::env::temp_dir().join(format!("echomate-test-{}.db", next_id("repo")));
        let repo = MemoryRepository::new(path.clone()).expect("repo");
        let contact = repo
            .upsert_contact(&ContactInput {
                id: None,
                alias: "联系人B".to_string(),
                channel: "wechat".to_string(),
                is_allowlisted: true,
            })
            .expect("upsert contact");
        let run = repo
            .record_suggestion_run(
                &contact.id,
                "codex",
                "clipboard",
                None,
                &[],
                "候选记忆测试。",
            )
            .expect("record run");
        repo.record_memory_candidates_for_run(
            &contact.id,
            &run.id,
            None,
            &[MemoryCandidate {
                memory_type: "event".to_string(),
                summary: "对方下周考试".to_string(),
                value: "对方下周考试".to_string(),
                source_kind: "clipboard".to_string(),
                source_ref: "current-request".to_string(),
                source_excerpt: "下周考试".to_string(),
                source_quote: "下周考试".to_string(),
                reason: "明确短期事件".to_string(),
                confidence: 0.82,
                sensitivity: "normal".to_string(),
                expires_at: String::new(),
                ttl_days: Some(10),
            }],
        )
        .expect("record candidate");
        let inbox = repo
            .list_memory_candidates(&contact.id, Some("candidate"), 10)
            .expect("list inbox");
        let saved = repo
            .confirm_memory_candidate_with_edits(&EditedMemoryCandidate {
                id: inbox[0].id.clone(),
                memory_type: "stress_point".to_string(),
                value: "对方最近备考压力较大".to_string(),
                source_excerpt: "说到下周考试".to_string(),
                sensitivity: "medium".to_string(),
                ttl_days: Some(3),
                clear_ttl: false,
            })
            .expect("confirm edited candidate");
        assert_eq!(saved.memory_type, "stress_point");
        assert_eq!(saved.value, "对方最近备考压力较大");
        assert_eq!(saved.sensitivity, "medium");
        assert!(!saved.expires_at.is_empty());
        assert!(repo
            .list_memory_candidates(&contact.id, Some("candidate"), 10)
            .expect("inbox after confirm")
            .is_empty());

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn memory_usage_tracking_records_candidate_refs() {
        let path = std::env::temp_dir().join(format!("echomate-test-{}.db", next_id("repo")));
        let repo = MemoryRepository::new(path.clone()).expect("repo");
        let contact = repo
            .upsert_contact(&ContactInput {
                id: None,
                alias: "联系人C".to_string(),
                channel: "wechat".to_string(),
                is_allowlisted: true,
            })
            .expect("upsert contact");
        let memory = repo
            .save_memory_candidate_for_contact(
                Some(&contact.id),
                &MemoryCandidate {
                    memory_type: "preference".to_string(),
                    summary: "对方喜欢轻松散步".to_string(),
                    value: "对方喜欢轻松散步".to_string(),
                    source_kind: "manual".to_string(),
                    source_ref: "manual-note".to_string(),
                    source_excerpt: "用户确认偏好".to_string(),
                    source_quote: "用户确认偏好".to_string(),
                    reason: "已确认偏好".to_string(),
                    confidence: 0.9,
                    sensitivity: "normal".to_string(),
                    expires_at: String::new(),
                    ttl_days: None,
                },
            )
            .expect("save memory");
        let run = repo
            .record_suggestion_run(&contact.id, "codex", "topic", None, &[], "主动找话题。")
            .expect("record run");
        let inserted = repo
            .record_candidate_memory_usage(
                &contact.id,
                &run.id,
                &[Candidate {
                    text: "这两天要不要找个地方散步".to_string(),
                    intent_group: "邀约".to_string(),
                    style_tags: vec!["低压".to_string()],
                    risk_flags: vec!["none".to_string()],
                    source_refs: vec![format!("memory:{}", memory.id)],
                    reason: "引用已确认偏好".to_string(),
                }],
            )
            .expect("record usage");
        assert_eq!(inserted, 1);
        let memories = repo
            .confirmed_memories_for_contact(&contact.id, 5)
            .expect("memories");
        assert!(!memories[0].last_used_at.is_empty());
        let card = repo
            .relationship_card(&contact.id)
            .expect("relationship card");
        let usage = card
            .memory_usages
            .iter()
            .find(|usage| usage.memory_id == memory.id)
            .expect("usage summary");
        assert_eq!(usage.usage_count, 1);
        assert!(!usage.last_used_at.is_empty());
        assert_eq!(usage.recent_references[0], "这两天要不要找个地方散步");

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn contacts_messages_retention_and_style_profile_work() {
        let path = std::env::temp_dir().join(format!("echomate-test-{}.db", next_id("repo")));
        let repo = MemoryRepository::new(path.clone()).expect("repo");

        let contact = repo
            .upsert_contact(&ContactInput {
                id: None,
                alias: "测试联系人A".to_string(),
                channel: "wechat".to_string(),
                is_allowlisted: true,
            })
            .expect("upsert contact");
        assert!(contact.is_allowlisted);
        assert!(repo
            .find_allowlisted_contact("测试联系人A", "wechat")
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
        assert_eq!(
            repo.message_event_count(&contact.id)
                .expect("message event count"),
            2
        );

        let source = repo
            .insert_source_context(
                &contact.id,
                "codex",
                "clipboard",
                "clipboard",
                "当前剪贴板文本",
                "我明天面试，有点紧张",
                Some("2026-06-09T08:00:00Z"),
                None,
                Some("unknown"),
                0.6,
                "{}",
            )
            .expect("insert source context");
        let source_cards = repo
            .recent_source_cards(&contact.id, 5)
            .expect("recent source cards");
        assert_eq!(source_cards.len(), 1);
        assert_eq!(source_cards[0].source_kind, "clipboard");

        let source_cards_for_run = source_cards.clone();
        let run = repo
            .record_suggestion_run(
                &contact.id,
                "codex",
                "clipboard",
                Some(&source.id),
                &source_cards_for_run,
                "对方提到明天面试。",
            )
            .expect("record suggestion run");
        assert_eq!(run.fact_source, "clipboard");
        assert_eq!(run.captured_at, "2026-06-09T08:00:00Z");
        assert_eq!(run.inferred_chat_time, "unknown");
        assert_eq!(run.source_confidence, 0.6);
        assert_eq!(
            repo.suggestion_run_count(&contact.id)
                .expect("suggestion run count"),
            1
        );
        let inserted_candidates = repo
            .record_memory_candidates_for_run(
                &contact.id,
                &run.id,
                Some(&source.id),
                &[MemoryCandidate {
                    memory_type: "event".to_string(),
                    summary: "她明天有面试".to_string(),
                    value: "她明天有面试".to_string(),
                    source_kind: "clipboard".to_string(),
                    source_ref: "current-request".to_string(),
                    source_excerpt: "我明天面试".to_string(),
                    source_quote: "我明天面试".to_string(),
                    reason: "明确提到明天面试".to_string(),
                    confidence: 0.86,
                    sensitivity: "normal".to_string(),
                    expires_at: String::new(),
                    ttl_days: Some(7),
                }],
            )
            .expect("record memory candidates");
        assert_eq!(inserted_candidates, 1);
        let inbox = repo
            .list_memory_candidates(&contact.id, Some("candidate"), 10)
            .expect("list memory candidate inbox");
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].reason, "明确提到明天面试");
        let confirmed_from_inbox = repo
            .confirm_memory_candidate(&inbox[0].id)
            .expect("confirm memory candidate");
        assert_eq!(confirmed_from_inbox.contact_id, contact.id);
        assert!(repo
            .list_memory_candidates(&contact.id, Some("candidate"), 10)
            .expect("candidate inbox after confirm")
            .is_empty());
        assert_eq!(
            repo.memory_candidate_count(&contact.id)
                .expect("memory candidate count"),
            1
        );

        let saved_facts = repo
            .save_contact_facts(
                &contact.id,
                &[
                    ContactFactCandidate {
                        fact_type: "age_band".to_string(),
                        value: "90 后".to_string(),
                        normalized_value: "90s".to_string(),
                        source_note: "联系人A 90 后，A 市人，在 B 市工作".to_string(),
                        fact_source: "manual".to_string(),
                        sensitivity: "normal".to_string(),
                        confidence: 0.9,
                        ttl_days: None,
                        usage_policy: "contextual".to_string(),
                    },
                    ContactFactCandidate {
                        fact_type: "hometown".to_string(),
                        value: "A 市".to_string(),
                        normalized_value: "A 市".to_string(),
                        source_note: "联系人A 90 后，A 市人，在 B 市工作".to_string(),
                        fact_source: "manual".to_string(),
                        sensitivity: "normal".to_string(),
                        confidence: 0.88,
                        ttl_days: None,
                        usage_policy: "contextual".to_string(),
                    },
                    ContactFactCandidate {
                        fact_type: "temporary_state".to_string(),
                        value: "最近健康状态不明".to_string(),
                        normalized_value: "health-unknown".to_string(),
                        source_note: "测试高敏过滤".to_string(),
                        fact_source: "manual".to_string(),
                        sensitivity: "high".to_string(),
                        confidence: 0.7,
                        ttl_days: Some(14),
                        usage_policy: "rare".to_string(),
                    },
                ],
            )
            .expect("save contact facts");
        assert_eq!(saved_facts.len(), 3);
        assert_eq!(
            repo.contact_fact_count(&contact.id)
                .expect("contact fact count"),
            3
        );
        assert_eq!(
            repo.recent_messages(&contact.id, 10)
                .expect("recent after manual facts")
                .len(),
            2,
            "manual facts must not be written to messages"
        );
        let prompt_facts = repo
            .prompt_contact_facts(&contact.id, 10)
            .expect("prompt facts");
        assert_eq!(prompt_facts.len(), 2);
        assert!(prompt_facts.iter().all(|fact| fact.fact_source == "manual"));
        repo.insert_screenshot_analysis(
            &contact.id,
            Some(&source.id),
            "/tmp/echomate-fake-screenshot.png",
            640,
            960,
            "test-ocr",
            &ScreenshotAnalysis {
                turns: vec![crate::domain::ScreenshotTurn {
                    speaker: "other".to_string(),
                    text: "我明天面试".to_string(),
                    media_kind: "text".to_string(),
                    visible_time_label: "昨天 22:53".to_string(),
                    bbox: Some(crate::domain::BoundingBox {
                        x: 0.1,
                        y: 0.2,
                        width: 0.4,
                        height: 0.05,
                    }),
                    confidence: 0.9,
                    warnings: Vec::new(),
                }],
                last_reply_target: "我明天面试".to_string(),
                visible_time_label: "昨天 22:53".to_string(),
                inferred_chat_time: "visible_time_label:昨天 22:53".to_string(),
                staleness: "visible_time_only".to_string(),
                warnings: Vec::new(),
            },
        )
        .expect("insert screenshot analysis");

        let saved = repo
            .save_memory_candidate_for_contact(
                Some(&contact.id),
                &MemoryCandidate {
                    memory_type: "event".to_string(),
                    summary: "她明天有面试".to_string(),
                    value: "她明天有面试".to_string(),
                    source_kind: "notification".to_string(),
                    source_ref: "toast".to_string(),
                    source_excerpt: "我明天面试".to_string(),
                    source_quote: "我明天面试".to_string(),
                    reason: "明确事件".to_string(),
                    confidence: 0.9,
                    sensitivity: "normal".to_string(),
                    expires_at: String::new(),
                    ttl_days: None,
                },
            )
            .expect("save scoped memory");
        assert_eq!(saved.contact_id, contact.id);
        assert_eq!(
            repo.confirmed_memories_for_contact(&contact.id, 5)
                .expect("memories")
                .len(),
            2
        );
        let relationship = repo
            .relationship_card(&contact.id)
            .expect("relationship card");
        assert_eq!(relationship.contact.id, contact.id);
        assert!(!relationship.contact_facts.is_empty());
        assert!(!relationship.memories.is_empty());
        let audit = repo
            .data_audit_report(&contact.id, 30)
            .expect("audit with screenshot");
        assert!(audit
            .counts
            .iter()
            .any(|item| item.table_name == "screenshot_analyses" && item.count == 1));

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
        assert_eq!(
            repo.contact_fact_count(&contact.id)
                .expect("fact count after clear"),
            0
        );
        assert_eq!(
            repo.message_event_count(&contact.id)
                .expect("message event count after clear"),
            0
        );
        assert_eq!(
            repo.memory_candidate_count(&contact.id)
                .expect("memory candidate count after clear"),
            0
        );
        assert!(repo
            .recent_source_cards(&contact.id, 10)
            .expect("source cards after clear")
            .is_empty());
        assert_eq!(
            repo.suggestion_run_count(&contact.id)
                .expect("suggestion run count after clear"),
            0
        );
        assert!(repo
            .rebuild_style_profile_from_adopted_replies()
            .expect("empty style rebuild")
            .is_none());

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn contamination_scanner_flags_test_artifacts_without_returning_full_text() {
        let path = std::env::temp_dir().join(format!("echomate-test-{}.db", next_id("repo")));
        let repo = MemoryRepository::new(path.clone()).expect("repo");

        let contact = repo
            .upsert_contact(&ContactInput {
                id: None,
                alias: "测试联系人A".to_string(),
                channel: "wechat".to_string(),
                is_allowlisted: true,
            })
            .expect("upsert contact");
        let source = repo
            .insert_source_context(
                &contact.id,
                "e2e-mock",
                "clipboard",
                "clipboard",
                "E2E mock source",
                "synthetic fixture text",
                None,
                None,
                Some("unknown"),
                0.5,
                "{}",
            )
            .expect("insert mock source context");
        repo.append_message_with_source_context(
            &contact.id,
            "other",
            "synthetic fixture text",
            "clipboard",
            false,
            Some(&source.id),
            Some(&source.captured_at),
            None,
            Some("unknown"),
            0.5,
        )
        .expect("append mock message");

        let findings = repo.scan_for_test_artifacts().expect("scan test artifacts");
        assert!(findings.iter().any(
            |finding| finding.table_name == "contacts" && finding.matched_text == "测试联系人"
        ));
        assert!(findings
            .iter()
            .any(|finding| finding.table_name == "source_contexts"
                && matches!(finding.matched_text.as_str(), "e2e" | "mock" | "e2e-mock")));
        assert!(
            findings
                .iter()
                .all(|finding| finding.matched_text != "synthetic fixture text"),
            "scanner should report only the marker, not full stored text"
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn e2e_mock_default_db_uses_temp_profile() {
        let _guard = ENV_LOCK.lock().expect("env lock");
        let old_mock = std::env::var("ECHOMATE_E2E_MOCK_PROVIDER").ok();
        let old_profile = std::env::var("ECHOMATE_E2E_PROFILE_DIR").ok();
        let profile =
            std::env::temp_dir().join(format!("echomate-e2e-profile-test-{}", next_id("repo")));
        std::env::set_var("ECHOMATE_E2E_MOCK_PROVIDER", "1");
        std::env::set_var("ECHOMATE_E2E_PROFILE_DIR", &profile);

        let path = default_db_path();
        assert!(path.starts_with(&profile));
        assert!(path.ends_with("echomate.db"));

        match old_mock {
            Some(value) => std::env::set_var("ECHOMATE_E2E_MOCK_PROVIDER", value),
            None => std::env::remove_var("ECHOMATE_E2E_MOCK_PROVIDER"),
        }
        match old_profile {
            Some(value) => std::env::set_var("ECHOMATE_E2E_PROFILE_DIR", value),
            None => std::env::remove_var("ECHOMATE_E2E_PROFILE_DIR"),
        }
    }
}
