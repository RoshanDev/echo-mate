// Codex CLI adapter

use crate::domain::CandidateEnvelope;

pub struct CodexProvider {
    // TODO: implement Codex CLI integration
}

impl CodexProvider {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn generate(&self, _prompt: &str, _schema_file: &str) -> anyhow::Result<CandidateEnvelope> {
        todo!("implement Codex CLI call")
    }
}
