/// Returns the JSON Schema for the candidate output format.
/// Both Claude (--json-schema) and Codex (--output-schema) use this.
pub fn candidate_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["candidates"],
        "properties": {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_schema_required_matches_top_level_properties() {
        let schema = candidate_schema();
        let properties = schema["properties"].as_object().expect("properties object");
        let required = schema["required"].as_array().expect("required array");
        let required = required
            .iter()
            .map(|value| value.as_str().expect("required string"))
            .collect::<std::collections::BTreeSet<_>>();

        assert_eq!(properties.len(), required.len());
        for key in properties.keys() {
            assert!(
                required.contains(key.as_str()),
                "missing required key {key}"
            );
        }
    }
}
