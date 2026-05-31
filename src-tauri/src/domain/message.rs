use serde::{Deserialize, Serialize};

use super::memory_item::{ContextSummaryCandidate, MemoryCandidate, NextAction, ReminderCandidate};

/// A raw chat message captured from clipboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub contact_id: String,
    pub text: String,
    pub is_mine: bool,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// A single AI-generated reply candidate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub text: String,
    #[serde(default)]
    pub style_tags: Vec<String>,
    #[serde(default)]
    pub risk_flags: Vec<String>,
    #[serde(default)]
    pub reason: String,
}

/// Envelope containing 5 candidates from the LLM provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateEnvelope {
    pub candidates: Vec<Candidate>,
    #[serde(default)]
    pub action_card: NextAction,
    #[serde(default)]
    pub memory_candidates: Vec<MemoryCandidate>,
    #[serde(default)]
    pub reminder_candidates: Vec<ReminderCandidate>,
    #[serde(default)]
    pub context_summary: ContextSummaryCandidate,
}

impl CandidateEnvelope {
    pub fn from_candidates(candidates: Vec<Candidate>) -> Self {
        Self {
            candidates,
            action_card: NextAction::default(),
            memory_candidates: Vec::new(),
            reminder_candidates: Vec::new(),
            context_summary: ContextSummaryCandidate::default(),
        }
    }
}

/// An event recording which candidate the user sent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendEvent {
    pub id: String,
    pub message_id: String,
    pub candidate_index: usize,
    pub provider: String,
    pub latency_ms: u64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
