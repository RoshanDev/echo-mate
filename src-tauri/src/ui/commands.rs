// Tauri commands exposed to the frontend

#[tauri::command]
pub async fn generate_replies(text: String) -> Result<String, String> {
    // TODO: wire up to orchestrator
    Ok(format!("Received: {}", text))
}
