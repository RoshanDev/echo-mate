use serde::{Deserialize, Serialize};

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
