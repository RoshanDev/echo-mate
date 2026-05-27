// Claude Code CLI adapter

use crate::domain::CandidateEnvelope;

pub struct ClaudeProvider {
    // TODO: implement Claude CLI integration
}

impl ClaudeProvider {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn generate(&self, _prompt: &str, _schema: &str) -> anyhow::Result<CandidateEnvelope> {
        todo!("implement Claude CLI call")
    }
}
