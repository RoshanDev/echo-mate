use crate::agent::schema;
use crate::domain::CandidateEnvelope;
use chrono::DateTime;

/// Validate candidate envelope against the JSON Schema
pub struct OutputParser;

impl OutputParser {
    pub fn new() -> Self {
        Self
    }

    /// Validate and normalize the candidate envelope
    pub fn validate(&self, envelope: &CandidateEnvelope) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if envelope.candidates.is_empty() {
            errors.push("No candidates returned".into());
            return Err(errors);
        }

        if envelope.candidates.len() != 5 {
            errors.push(format!(
                "Expected 5 candidates, got {}",
                envelope.candidates.len()
            ));
        }

        for (i, candidate) in envelope.candidates.iter().enumerate() {
            if candidate.text.is_empty() {
                errors.push(format!("Candidate {}: empty text", i + 1));
            }
            if candidate.text.len() > 500 {
                errors.push(format!(
                    "Candidate {}: text too long ({} chars)",
                    i + 1,
                    candidate.text.len()
                ));
            }
        }

        const ACTION_TYPES: &[&str] = &[
            "continue_chat",
            "wrap_up",
            "light_follow_up",
            "do_not_push",
            "safe_repair",
            "soft_invite_candidate",
        ];
        if !ACTION_TYPES.contains(&envelope.action_card.action_type.as_str()) {
            errors.push(format!(
                "Unknown action type: {}",
                envelope.action_card.action_type
            ));
        }
        if !(0.0..=1.0).contains(&envelope.action_card.confidence) {
            errors.push("Action confidence must be between 0 and 1".into());
        }

        const MEMORY_TYPES: &[&str] = &[
            "event",
            "preference",
            "boundary",
            "stress_point",
            "relationship_milestone",
        ];
        const SENSITIVITY_LEVELS: &[&str] = &["normal", "medium", "high", "forbidden"];
        for (i, memory) in envelope.memory_candidates.iter().enumerate() {
            if !MEMORY_TYPES.contains(&memory.memory_type.as_str()) {
                errors.push(format!(
                    "Memory candidate {}: unknown type {}",
                    i + 1,
                    memory.memory_type
                ));
            }
            if !SENSITIVITY_LEVELS.contains(&memory.sensitivity.as_str()) {
                errors.push(format!(
                    "Memory candidate {}: unknown sensitivity {}",
                    i + 1,
                    memory.sensitivity
                ));
            }
            if memory.source_ref.is_empty() && memory.source_excerpt.is_empty() {
                errors.push(format!(
                    "Memory candidate {}: missing source_ref/source_excerpt",
                    i + 1
                ));
            }
        }

        for (i, reminder) in envelope.reminder_candidates.iter().enumerate() {
            if !MEMORY_TYPES.contains(&reminder.memory_type.as_str()) {
                errors.push(format!(
                    "Reminder candidate {}: unknown memory type {}",
                    i + 1,
                    reminder.memory_type
                ));
            }
            if !SENSITIVITY_LEVELS.contains(&reminder.sensitivity.as_str()) {
                errors.push(format!(
                    "Reminder candidate {}: unknown sensitivity {}",
                    i + 1,
                    reminder.sensitivity
                ));
            }
            if !reminder.trigger_at.is_empty()
                && DateTime::parse_from_rfc3339(&reminder.trigger_at).is_err()
            {
                errors.push(format!(
                    "Reminder candidate {}: trigger_at is not RFC3339",
                    i + 1
                ));
            }
        }

        if !errors.is_empty() {
            Err(errors)
        } else {
            Ok(())
        }
    }

    /// Check if we should use the schema directly for validation
    pub fn validate_with_schema(&self, output: &str) -> Result<CandidateEnvelope, String> {
        let _schema = schema::candidate_schema();

        // Parse as JSON value first
        let value: serde_json::Value =
            serde_json::from_str(output).map_err(|e| format!("Invalid JSON: {}", e))?;

        // Try to extract candidates from various wrappers
        let candidates_value = if let Some(c) = value.get("candidates") {
            c.clone()
        } else if let Some(inner) = value.get("structured_output") {
            inner
                .get("candidates")
                .cloned()
                .unwrap_or(serde_json::Value::Null)
        } else {
            serde_json::Value::Null
        };

        if candidates_value.is_null() {
            return Err("No 'candidates' field found in output".into());
        }

        // Deserialize
        let envelope: CandidateEnvelope =
            serde_json::from_value(value).map_err(|e| format!("Schema mismatch: {}", e))?;

        Ok(envelope)
    }
}
