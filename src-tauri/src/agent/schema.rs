/// Returns the JSON Schema for the candidate output format.
/// Both Claude (--json-schema) and Codex (--output-schema) use this.
pub fn candidate_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "candidates",
            "situation",
            "action_card",
            "source_summary",
            "memory_candidates",
            "reminder_candidates",
            "context_summary",
            "screenshot_analysis"
        ],
        "properties": {
            "candidates": {
                "type": "array",
                "minItems": 5,
                "maxItems": 5,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["text", "intent_group", "style_tags", "risk_flags", "source_refs", "reason"],
                    "properties": {
                        "text": {
                            "type": "string",
                            "minLength": 2,
                            "maxLength": 120,
                            "description": "The candidate reply text in Chinese"
                        },
                        "intent_group": {
                            "type": "string",
                            "enum": ["稳妥", "轻松", "幽默", "温柔", "收束", "邀约", "支持"]
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
                        "source_refs": {
                            "type": "array",
                            "items": { "type": "string", "maxLength": 80 },
                            "maxItems": 5
                        },
                        "reason": {
                            "type": "string",
                            "maxLength": 160,
                            "description": "Why this reply was chosen"
                        }
                    }
                }
            },
            "situation": {
                "type": "object",
                "additionalProperties": false,
                "required": ["summary", "action_type", "staleness", "relationship_signal", "confidence"],
                "properties": {
                    "summary": { "type": "string", "maxLength": 220 },
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
                    "staleness": {
                        "type": "string",
                        "enum": ["fresh", "stale", "unknown", "visible_time_only", "inferred"]
                    },
                    "relationship_signal": {
                        "type": "string",
                        "maxLength": 160
                    },
                    "confidence": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1
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
            "source_summary": {
                "type": "string",
                "maxLength": 260
            },
            "memory_candidates": {
                "type": "array",
                "maxItems": 5,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": [
                        "memory_type",
                        "summary",
                        "value",
                        "source_kind",
                        "source_ref",
                        "source_excerpt",
                        "source_quote",
                        "reason",
                        "confidence",
                        "sensitivity",
                        "expires_at",
                        "ttl_days"
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
                        "summary": {
                            "type": "string",
                            "maxLength": 180
                        },
                        "value": {
                            "type": "string",
                            "minLength": 2,
                            "maxLength": 180
                        },
                        "source_kind": {
                            "type": "string",
                            "enum": ["clipboard", "screenshot", "notification", "manual", "topic"]
                        },
                        "source_ref": {
                            "type": "string",
                            "maxLength": 120
                        },
                        "source_excerpt": {
                            "type": "string",
                            "maxLength": 180
                        },
                        "source_quote": {
                            "type": "string",
                            "maxLength": 180
                        },
                        "reason": {
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
                        },
                        "ttl_days": {
                            "type": ["integer", "null"],
                            "minimum": 1,
                            "maximum": 3650
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
                        "kind",
                        "memory_type",
                        "memory_value",
                        "source_kind",
                        "source_ref",
                        "source_excerpt",
                        "recommended_time",
                        "trigger_at",
                        "reason",
                        "suggested_follow_up",
                        "source_context_id",
                        "cooldown_key",
                        "confidence",
                        "sensitivity"
                    ],
                    "properties": {
                        "kind": {
                            "type": "string",
                            "enum": ["follow_up", "check_in", "important_date", "custom"]
                        },
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
                            "enum": ["clipboard", "screenshot", "notification", "manual", "topic"]
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
                        "source_context_id": {
                            "type": "string",
                            "maxLength": 120
                        },
                        "cooldown_key": {
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
                        "enum": ["clipboard", "screenshot", "notification", "manual", "topic"]
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
            },
            "screenshot_analysis": {
                "type": "object",
                "additionalProperties": false,
                "required": [
                    "turns",
                    "last_reply_target",
                    "visible_time_label",
                    "inferred_chat_time",
                    "staleness",
                    "warnings"
                ],
                "properties": {
                    "turns": {
                        "type": "array",
                        "maxItems": 20,
                        "items": {
                            "type": "object",
                            "additionalProperties": false,
                            "required": [
                                "speaker",
                                "text",
                                "media_kind",
                                "visible_time_label",
                                "bbox",
                                "confidence",
                                "warnings"
                            ],
                            "properties": {
                                "speaker": {
                                    "type": "string",
                                    "enum": ["me", "other", "system", "unknown"]
                                },
                                "text": {
                                    "type": "string",
                                    "maxLength": 220
                                },
                                "media_kind": {
                                    "type": "string",
                                    "enum": ["text", "image", "emoji", "quote", "system", "unknown"]
                                },
                                "visible_time_label": {
                                    "type": "string",
                                    "maxLength": 80
                                },
                                "bbox": {
                                    "type": "object",
                                    "additionalProperties": false,
                                    "required": ["x", "y", "width", "height"],
                                    "properties": {
                                        "x": { "type": "number" },
                                        "y": { "type": "number" },
                                        "width": { "type": "number" },
                                        "height": { "type": "number" }
                                    }
                                },
                                "confidence": {
                                    "type": "number",
                                    "minimum": 0,
                                    "maximum": 1
                                },
                                "warnings": {
                                    "type": "array",
                                    "items": { "type": "string", "maxLength": 120 },
                                    "maxItems": 5
                                }
                            }
                        }
                    },
                    "last_reply_target": {
                        "type": "string",
                        "maxLength": 220
                    },
                    "visible_time_label": {
                        "type": "string",
                        "maxLength": 80
                    },
                    "inferred_chat_time": {
                        "type": "string",
                        "maxLength": 80
                    },
                    "staleness": {
                        "type": "string",
                        "enum": ["fresh", "stale", "unknown", "visible_time_only", "inferred"]
                    },
                    "warnings": {
                        "type": "array",
                        "items": { "type": "string", "maxLength": 160 },
                        "maxItems": 8
                    }
                }
            }
        }
    })
}

/// JSON Schema for classifying user-entered contact notes into structured facts.
pub fn contact_fact_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["facts", "warnings", "usage_guidance"],
        "properties": {
            "facts": {
                "type": "array",
                "maxItems": 8,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": [
                        "fact_type",
                        "value",
                        "normalized_value",
                        "source_note",
                        "fact_source",
                        "sensitivity",
                        "confidence",
                        "ttl_days",
                        "usage_policy"
                    ],
                    "properties": {
                        "fact_type": {
                            "type": "string",
                            "enum": [
                                "birth_year",
                                "age_band",
                                "hometown",
                                "current_city",
                                "work_city",
                                "occupation",
                                "preference",
                                "boundary",
                                "important_date",
                                "temporary_state",
                                "note"
                            ]
                        },
                        "value": {
                            "type": "string",
                            "minLength": 1,
                            "maxLength": 120
                        },
                        "normalized_value": {
                            "type": "string",
                            "maxLength": 120
                        },
                        "source_note": {
                            "type": "string",
                            "maxLength": 180
                        },
                        "fact_source": {
                            "type": "string",
                            "enum": ["manual"]
                        },
                        "sensitivity": {
                            "type": "string",
                            "enum": ["normal", "medium", "high", "forbidden"]
                        },
                        "confidence": {
                            "type": "number",
                            "minimum": 0,
                            "maximum": 1
                        },
                        "ttl_days": {
                            "type": ["integer", "null"],
                            "minimum": 1,
                            "maximum": 3650
                        },
                        "usage_policy": {
                            "type": "string",
                            "enum": ["contextual", "rare", "reminder_only", "never"]
                        }
                    }
                }
            },
            "warnings": {
                "type": "array",
                "items": { "type": "string", "maxLength": 160 },
                "maxItems": 5
            },
            "usage_guidance": {
                "type": "string",
                "maxLength": 260
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

    #[test]
    fn contact_fact_schema_has_strict_top_level() {
        let schema = contact_fact_schema();
        assert_eq!(schema["additionalProperties"], false);
        let required = schema["required"].as_array().expect("required array");
        assert!(required.iter().any(|value| value == "facts"));
        assert!(required.iter().any(|value| value == "warnings"));
        assert!(required.iter().any(|value| value == "usage_guidance"));
    }
}
