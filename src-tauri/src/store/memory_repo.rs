// Memory repository for style profiles, contact facts, and reminder MVP data.
use crate::domain::{
    Candidate, ContextSummaryCandidate, ContextSummaryRecord, MemoryCandidate, MemoryItemRecord,
    NextAction, ReminderCandidate, ReminderDetail, ReminderRecord, ReplyFeedbackRecord,
};
use crate::store::migrations::run_migrations;
use anyhow::{anyhow, bail};
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct MemoryRepository {
    db_path: PathBuf,
}

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

    pub fn insert_context_summary(
        &self,
        summary: &ContextSummaryCandidate,
    ) -> anyhow::Result<ContextSummaryRecord> {
        if summary.summary.trim().is_empty() {
            bail!("context summary is empty");
        }

        let record = ContextSummaryRecord {
            id: next_id("ctx"),
            source_kind: non_empty(&summary.source_kind, "text"),
            source_ref: summary.source_ref.clone(),
            summary: summary.summary.clone(),
            created_at: now_rfc3339(),
        };

        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO context_summary (id, source_kind, source_ref, summary, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                &record.id,
                &record.source_kind,
                &record.source_ref,
                &record.summary,
                &record.created_at
            ],
        )?;
        Ok(record)
    }

    pub fn save_memory_candidate(
        &self,
        candidate: &MemoryCandidate,
    ) -> anyhow::Result<MemoryItemRecord> {
        ensure_allowed_sensitivity(&candidate.sensitivity)?;
        if candidate.value.trim().is_empty() {
            bail!("memory value is empty");
        }
        let record = MemoryItemRecord {
            id: next_id("mem"),
            memory_type: non_empty(&candidate.memory_type, "event"),
            value: candidate.value.trim().to_string(),
            source_kind: non_empty(&candidate.source_kind, "text"),
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
        ensure_allowed_sensitivity(&candidate.sensitivity)?;
        if candidate.memory_value.trim().is_empty() {
            bail!("reminder memory value is empty");
        }

        let memory = MemoryItemRecord {
            id: next_id("mem"),
            memory_type: non_empty(&candidate.memory_type, "event"),
            value: candidate.memory_value.trim().to_string(),
            source_kind: non_empty(&candidate.source_kind, "text"),
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
                m.id, m.type, m.value, m.source_kind, m.source_ref, m.source_excerpt,
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
            let memory = MemoryItemRecord {
                id: row.get(9)?,
                memory_type: row.get(10)?,
                value: row.get(11)?,
                source_kind: row.get(12)?,
                source_ref: row.get(13)?,
                source_excerpt: row.get(14)?,
                confidence: row.get(15)?,
                sensitivity: row.get(16)?,
                expires_at: row.get(17)?,
                status: row.get(18)?,
                created_at: row.get(19)?,
                updated_at: row.get(20)?,
            };
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
                m.id, m.type, m.value, m.source_kind, m.source_ref, m.source_excerpt,
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
            let memory = MemoryItemRecord {
                id: row.get(9)?,
                memory_type: row.get(10)?,
                value: row.get(11)?,
                source_kind: row.get(12)?,
                source_ref: row.get(13)?,
                source_excerpt: row.get(14)?,
                confidence: row.get(15)?,
                sensitivity: row.get(16)?,
                expires_at: row.get(17)?,
                status: row.get(18)?,
                created_at: row.get(19)?,
                updated_at: row.get(20)?,
            };
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
        let record = ReplyFeedbackRecord {
            id: next_id("fb"),
            generation_id: generation_id.to_string(),
            action: action.to_string(),
            candidate_index,
            created_at: now_rfc3339(),
        };
        let conn = self.connection()?;
        conn.execute(
            "INSERT INTO reply_feedback (id, generation_id, action, candidate_index, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                &record.id,
                &record.generation_id,
                &record.action,
                record.candidate_index,
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
                (id, type, value, source_kind, source_ref, source_excerpt, confidence, sensitivity, expires_at, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                &record.id,
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
                source_kind: "text".to_string(),
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
                    source_kind: "text".to_string(),
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
                source_kind: "text".to_string(),
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
}
