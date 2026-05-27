/// Returns the JSON Schema for the candidate output format.
/// Both Claude (--json-schema) and Codex (--output-schema) use this.
pub fn candidate_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["provider", "candidates"],
        "properties": {
            "provider": {
                "type": "string",
                "enum": ["codex", "claude"]
            },
            "conversation_summary": {
                "type": "string",
                "description": "Brief summary of the conversation context"
            },
            "candidates": {
                "type": "array",
                "minItems": 5,
                "maxItems": 5,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["text", "style_tags", "risk_flags", "reason"],
                    "properties": {
                        "text": {
                            "type": "string",
                            "minLength": 2,
                            "maxLength": 120,
                            "description": "The candidate reply text in Chinese"
                        },
                        "style_tags": {
                            "type": "array",
                            "items": { "type": "string" },
                            "maxItems": 5,
                            "description": "Style labels like '稳妥', '轻松', '温柔'"
                        },
                        "risk_flags": {
                            "type": "array",
                            "items": {
                                "type": "string",
                                "enum": ["none", "too_cold", "too_eager", "too_flirty", "assumption", "promise_risk"]
                            },
                            "maxItems": 4
                        },
                        "reason": {
                            "type": "string",
                            "maxLength": 160,
                            "description": "Why this reply was chosen"
                        }
                    }
                }
            }
        }
    })
}
