use crate::models::{TransferJobView, TransferKind};
use crate::services::transfer_queue;

/// Enqueue a transfer job. `kind` is one of "upload" | "download" | "uploadDir"
/// | "downloadDir". For uploads: src = local path, dst = remote path. For
/// downloads: src = remote path, dst = local path. Returns the new job id.
#[tauri::command]
pub fn enqueue_transfer(
    profile_id: String,
    kind: String,
    src: String,
    dst: String,
    filename: String,
) -> Result<u64, String> {
    let kind = match kind.as_str() {
        "upload" => TransferKind::Upload,
        "download" => TransferKind::Download,
        "uploadDir" => TransferKind::UploadDir,
        "downloadDir" => TransferKind::DownloadDir,
        other => return Err(format!("Unknown transfer kind: {}", other)),
    };
    Ok(transfer_queue::enqueue(profile_id, kind, src, dst, filename))
}

#[tauri::command]
pub fn cancel_transfer(job_id: u64) {
    transfer_queue::cancel(job_id);
}

#[tauri::command]
pub fn cancel_all_transfers() {
    transfer_queue::cancel_all();
}

#[tauri::command]
pub fn list_transfers() -> Vec<TransferJobView> {
    transfer_queue::list()
}

#[tauri::command]
pub fn clear_finished_transfers() {
    transfer_queue::clear_finished();
}
