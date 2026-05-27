use crate::domain::CandidateEnvelope;
use crate::agent::schema;

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

        if envelope.candidates.len() < 3 {
            errors.push(format!(
                "Expected at least 3 candidates, got {}",
                envelope.candidates.len()
            ));
        }

        for (i, candidate) in envelope.candidates.iter().enumerate() {
            if candidate.text.is_empty() {
                errors.push(format!("Candidate {}: empty text", i + 1));
            }
            if candidate.text.len() > 500 {
                errors.push(format!("Candidate {}: text too long ({} chars)", i + 1, candidate.text.len()));
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
        let value: serde_json::Value = serde_json::from_str(output)
            .map_err(|e| format!("Invalid JSON: {}", e))?;

        // Try to extract candidates from various wrappers
        let candidates_value = if let Some(c) = value.get("candidates") {
            c.clone()
        } else if let Some(inner) = value.get("structured_output") {
            inner.get("candidates").cloned().unwrap_or(serde_json::Value::Null)
        } else {
            serde_json::Value::Null
        };

        if candidates_value.is_null() {
            return Err("No 'candidates' field found in output".into());
        }

        // Deserialize
        let envelope: CandidateEnvelope = serde_json::from_value(value)
            .map_err(|e| format!("Schema mismatch: {}", e))?;

        Ok(envelope)
    }
}
