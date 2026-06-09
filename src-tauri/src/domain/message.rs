use serde::{Deserialize, Serialize};

use super::memory_item::{ContextSummaryCandidate, MemoryCandidate, NextAction, ReminderCandidate};

fn default_intent_group() -> String {
    "稳妥".to_string()
}

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
    #[serde(default = "default_intent_group")]
    pub intent_group: String,
    #[serde(default)]
    pub style_tags: Vec<String>,
    #[serde(default)]
    pub risk_flags: Vec<String>,
    #[serde(default)]
    pub source_refs: Vec<String>,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default)]
    pub width: f64,
    #[serde(default)]
    pub height: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotTurn {
    #[serde(default)]
    pub speaker: String,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub media_kind: String,
    #[serde(default)]
    pub visible_time_label: String,
    #[serde(default)]
    pub bbox: Option<BoundingBox>,
    #[serde(default)]
    pub confidence: f64,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotAnalysis {
    #[serde(default)]
    pub turns: Vec<ScreenshotTurn>,
    #[serde(default)]
    pub last_reply_target: String,
    #[serde(default)]
    pub visible_time_label: String,
    #[serde(default)]
    pub inferred_chat_time: String,
    #[serde(default)]
    pub staleness: String,
    #[serde(default)]
    pub warnings: Vec<String>,
}

impl Default for ScreenshotAnalysis {
    fn default() -> Self {
        Self {
            turns: Vec::new(),
            last_reply_target: String::new(),
            visible_time_label: String::new(),
            inferred_chat_time: "unknown".to_string(),
            staleness: "unknown".to_string(),
            warnings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationSituation {
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub action_type: String,
    #[serde(default)]
    pub staleness: String,
    #[serde(default)]
    pub relationship_signal: String,
    #[serde(default)]
    pub confidence: f64,
}

impl Default for GenerationSituation {
    fn default() -> Self {
        Self {
            summary: String::new(),
            action_type: "continue_chat".to_string(),
            staleness: "unknown".to_string(),
            relationship_signal: String::new(),
            confidence: 0.0,
        }
    }
}

/// Envelope containing 5 candidates from the LLM provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandidateEnvelope {
    pub candidates: Vec<Candidate>,
    #[serde(default)]
    pub situation: GenerationSituation,
    #[serde(default)]
    pub action_card: NextAction,
    #[serde(default)]
    pub source_summary: String,
    #[serde(default)]
    pub memory_candidates: Vec<MemoryCandidate>,
    #[serde(default)]
    pub reminder_candidates: Vec<ReminderCandidate>,
    #[serde(default)]
    pub context_summary: ContextSummaryCandidate,
    #[serde(default)]
    pub screenshot_analysis: ScreenshotAnalysis,
}

impl CandidateEnvelope {
    pub fn from_candidates(candidates: Vec<Candidate>) -> Self {
        Self {
            candidates,
            situation: GenerationSituation::default(),
            action_card: NextAction::default(),
            source_summary: String::new(),
            memory_candidates: Vec::new(),
            reminder_candidates: Vec::new(),
            context_summary: ContextSummaryCandidate::default(),
            screenshot_analysis: ScreenshotAnalysis::default(),
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
