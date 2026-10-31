//! Tauri commands for the AI access GUI. Only the GUI can create tokens or change
//! permissions — none of this is reachable through the AI socket.

use crate::models::ai::{AiCreatedToken, AiStatus, AiTokenInput, AiTokenView};
use crate::services::{ai_server, ai_token_service, launch_service, remote_copy};

#[tauri::command]
pub fn ai_status() -> AiStatus {
    ai_server::status()
}

#[tauri::command]
pub fn ai_set_enabled(enabled: bool) -> Result<AiStatus, String> {
    ai_token_service::set_enabled(enabled)?;
    if enabled {
        ai_server::start()?;
    } else {
        ai_server::stop();
    }
    Ok(ai_server::status())
}

#[tauri::command]
pub fn ai_list_tokens() -> Result<Vec<AiTokenView>, String> {
    ai_token_service::list()
}

#[tauri::command]
pub fn ai_create_token(input: AiTokenInput) -> Result<AiCreatedToken, String> {
    ai_token_service::create(input)
}

#[tauri::command]
pub fn ai_update_token(id: String, input: AiTokenInput) -> Result<AiTokenView, String> {
    ai_token_service::update(&id, input)
}

#[tauri::command]
pub fn ai_revoke_token(id: String) -> Result<(), String> {
    ai_token_service::revoke(&id)
}

#[tauri::command]
pub fn ai_bridge_command() -> Result<String, String> {
    Ok(format!("{} --mcp", remote_copy::shell_quote(&launch_service::exec_target()?)))
}

#[tauri::command]
pub fn ai_confirm_response(request_id: String, approved: bool) {
    ai_server::confirm_response(&request_id, approved);
}
