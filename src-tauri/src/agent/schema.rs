// JSON Schema definitions for structured output

/// Returns the JSON Schema for the candidate output format
pub fn candidate_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "candidates": {
                "type": "array",
                "minItems": 5,
                "maxItems": 5,
                "items": {
                    "type": "object",
                    "properties": {
                        "text": { "type": "string" },
                        "tone": { "type": "string" },
                        "strategy": { "type": "string" },
                        "risk": { "type": "string" }
                    },
                    "required": ["text", "tone", "strategy", "risk"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["candidates"],
        "additionalProperties": false
    })
}
