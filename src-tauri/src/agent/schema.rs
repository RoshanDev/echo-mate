/// Returns the JSON Schema for the candidate output format.
/// Both Claude (--json-schema) and Codex (--output-schema) use this.
pub fn candidate_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["candidates", "action_card", "memory_candidates", "reminder_candidates", "context_summary"],
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
            },
            "action_card": {
                "type": "object",
                "additionalProperties": false,
                "required": ["action_type", "reason", "confidence"],
                "properties": {
                    "action_type": {
                        "type": "string",
                        "enum": [
                            "continue_chat",
                            "wrap_up",
                            "light_follow_up",
                            "do_not_push",
                            "safe_repair",
                            "soft_invite_candidate"
                        ]
                    },
                    "reason": {
                        "type": "string",
                        "maxLength": 180
                    },
                    "confidence": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1
                    }
                }
            },
            "memory_candidates": {
                "type": "array",
                "maxItems": 5,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": [
                        "memory_type",
                        "value",
                        "source_kind",
                        "source_ref",
                        "source_excerpt",
                        "confidence",
                        "sensitivity",
                        "expires_at"
                    ],
                    "properties": {
                        "memory_type": {
                            "type": "string",
                            "enum": [
                                "event",
                                "preference",
                                "boundary",
                                "stress_point",
                                "relationship_milestone"
                            ]
                        },
                        "value": {
                            "type": "string",
                            "minLength": 2,
                            "maxLength": 180
                        },
                        "source_kind": {
                            "type": "string",
                            "enum": ["text", "screenshot"]
                        },
                        "source_ref": {
                            "type": "string",
                            "maxLength": 120
                        },
                        "source_excerpt": {
                            "type": "string",
                            "maxLength": 180
                        },
                        "confidence": {
                            "type": "number",
                            "minimum": 0,
                            "maximum": 1
                        },
                        "sensitivity": {
                            "type": "string",
                            "enum": ["normal", "medium", "high", "forbidden"]
                        },
                        "expires_at": {
                            "type": "string",
                            "maxLength": 40,
                            "description": "RFC3339 time when this memory should expire, or empty string if long-lived"
                        }
                    }
                }
            },
            "reminder_candidates": {
                "type": "array",
                "maxItems": 2,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": [
                        "memory_type",
                        "memory_value",
                        "source_kind",
                        "source_ref",
                        "source_excerpt",
                        "recommended_time",
                        "trigger_at",
                        "reason",
                        "suggested_follow_up",
                        "confidence",
                        "sensitivity"
                    ],
                    "properties": {
                        "memory_type": {
                            "type": "string",
                            "enum": [
                                "event",
                                "preference",
                                "boundary",
                                "stress_point",
                                "relationship_milestone"
                            ]
                        },
                        "memory_value": {
                            "type": "string",
                            "minLength": 2,
                            "maxLength": 180
                        },
                        "source_kind": {
                            "type": "string",
                            "enum": ["text", "screenshot"]
                        },
                        "source_ref": {
                            "type": "string",
                            "maxLength": 120
                        },
                        "source_excerpt": {
                            "type": "string",
                            "maxLength": 180
                        },
                        "recommended_time": {
                            "type": "string",
                            "maxLength": 80
                        },
                        "trigger_at": {
                            "type": "string",
                            "maxLength": 40,
                            "description": "RFC3339 local or UTC timestamp for the reminder"
                        },
                        "reason": {
                            "type": "string",
                            "maxLength": 180
                        },
                        "suggested_follow_up": {
                            "type": "string",
                            "maxLength": 120
                        },
                        "confidence": {
                            "type": "number",
                            "minimum": 0,
                            "maximum": 1
                        },
                        "sensitivity": {
                            "type": "string",
                            "enum": ["normal", "medium", "high", "forbidden"]
                        }
                    }
                }
            },
            "context_summary": {
                "type": "object",
                "additionalProperties": false,
                "required": ["source_kind", "source_ref", "summary"],
                "properties": {
                    "source_kind": {
                        "type": "string",
                        "enum": ["text", "screenshot"]
                    },
                    "source_ref": {
                        "type": "string",
                        "maxLength": 120
                    },
                    "summary": {
                        "type": "string",
                        "maxLength": 220
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

    #[test]
    fn candidate_schema_nested_objects_are_strict() {
        fn assert_required_matches_properties(value: &serde_json::Value) {
            if value["type"] == "object" {
                let properties = value["properties"].as_object().expect("properties object");
                let required = value["required"].as_array().expect("required array");
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

            if let Some(properties) = value["properties"].as_object() {
                for child in properties.values() {
                    assert_required_matches_properties(child);
                }
            }
            if let Some(items) = value.get("items") {
                assert_required_matches_properties(items);
            }
        }

        assert_required_matches_properties(&candidate_schema());
    }
}
