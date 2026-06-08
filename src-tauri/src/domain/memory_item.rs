use serde::{Deserialize, Serialize};

fn default_confidence() -> f64 {
    0.0
}

fn default_action_type() -> String {
    "continue_chat".to_string()
}

fn default_source_kind() -> String {
    "clipboard".to_string()
}

fn default_sensitivity() -> String {
    "normal".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextAction {
    #[serde(default = "default_action_type")]
    pub action_type: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
}

impl Default for NextAction {
    fn default() -> Self {
        Self {
            action_type: default_action_type(),
            reason: String::new(),
            confidence: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCandidate {
    #[serde(default)]
    pub memory_type: String,
    #[serde(default)]
    pub value: String,
    #[serde(default = "default_source_kind")]
    pub source_kind: String,
    #[serde(default)]
    pub source_ref: String,
    #[serde(default)]
    pub source_excerpt: String,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
    #[serde(default = "default_sensitivity")]
    pub sensitivity: String,
    #[serde(default)]
    pub expires_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReminderCandidate {
    #[serde(default)]
    pub memory_type: String,
    #[serde(default)]
    pub memory_value: String,
    #[serde(default = "default_source_kind")]
    pub source_kind: String,
    #[serde(default)]
    pub source_ref: String,
    #[serde(default)]
    pub source_excerpt: String,
    #[serde(default)]
    pub recommended_time: String,
    #[serde(default)]
    pub trigger_at: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub suggested_follow_up: String,
    #[serde(default = "default_confidence")]
    pub confidence: f64,
    #[serde(default = "default_sensitivity")]
    pub sensitivity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSummaryCandidate {
    #[serde(default = "default_source_kind")]
    pub source_kind: String,
    #[serde(default)]
    pub source_ref: String,
    #[serde(default)]
    pub summary: String,
}

impl Default for ContextSummaryCandidate {
    fn default() -> Self {
        Self {
            source_kind: default_source_kind(),
            source_ref: String::new(),
            summary: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryItemRecord {
    pub id: String,
    pub contact_id: String,
    pub memory_type: String,
    pub value: String,
    pub source_kind: String,
    pub source_ref: String,
    pub source_excerpt: String,
    pub confidence: f64,
    pub sensitivity: String,
    pub expires_at: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReminderRecord {
    pub id: String,
    pub memory_id: String,
    pub trigger_at: String,
    pub reason: String,
    pub suggested_follow_up: String,
    pub status: String,
    pub snooze_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSummaryRecord {
    pub id: String,
    pub contact_id: String,
    pub source_kind: String,
    pub source_ref: String,
    pub summary: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplyFeedbackRecord {
    pub id: String,
    pub generation_id: String,
    pub action: String,
    pub candidate_index: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReminderDetail {
    pub reminder: ReminderRecord,
    pub memory_item: MemoryItemRecord,
    pub action_card: NextAction,
    pub follow_up_candidates: Vec<super::message::Candidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactRecord {
    pub id: String,
    pub alias: String,
    pub channel: String,
    pub is_allowlisted: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactInput {
    #[serde(default)]
    pub id: Option<String>,
    pub alias: String,
    #[serde(default)]
    pub channel: String,
    #[serde(default)]
    pub is_allowlisted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRecord {
    pub id: String,
    pub contact_id: String,
    pub role: String,
    pub text: String,
    pub source: String,
    pub approved: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleProfileRecord {
    pub id: String,
    pub profile_json: String,
    pub sample_count: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPolicy {
    pub allowlisted: bool,
    pub contact_id: String,
    pub contact_alias: String,
    pub reason: String,
    pub can_save_context: bool,
    pub global_privacy_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionStatus {
    pub platform: String,
    pub windows_notification_helper_enabled: bool,
    pub windows_notification_available: bool,
    pub windows_notification_status: String,
    pub macos_context_helper_enabled: bool,
    pub macos_accessibility_enabled: bool,
    pub macos_context_status: String,
    pub fallback_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacosContextSnapshot {
    pub platform: String,
    pub available: bool,
    pub helper_enabled: bool,
    pub accessibility_enabled: bool,
    pub front_app: String,
    pub window_title: String,
    pub selected_text_available: bool,
    pub selected_text_excerpt: String,
    pub pasteboard_available: bool,
    pub pasteboard_excerpt: String,
    pub status: String,
    pub fallback_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformSignal {
    pub contact_alias: String,
    #[serde(default)]
    pub channel: String,
    #[serde(default = "default_source_kind")]
    pub source: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub app_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformSignalResult {
    pub allowed: bool,
    pub reason: String,
    pub contact: Option<ContactRecord>,
    pub message: Option<MessageRecord>,
}

/// A learned fact about the conversation contact
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContactFact {
    pub id: String,
    pub contact_id: String,
    pub category: String,
    pub content: String,
    pub confidence: f64,
    pub evidence_message_ids: Vec<String>,
    pub superseded_by: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// User's communication style profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleProfile {
    pub id: String,
    pub user_id: String,
    pub avg_sentence_length: f64,
    pub tone_labels: Vec<String>,
    pub emoji_usage: f64,
    pub common_phrases: Vec<String>,
    pub avoid_phrases: Vec<String>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
