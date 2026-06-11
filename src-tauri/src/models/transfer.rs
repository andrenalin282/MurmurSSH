use serde::{Deserialize, Serialize};

/// Sentinel returned from transfer functions when a cancel was requested.
/// The frontend recognises this value and renders a cancel-friendly status.
pub const CANCELLED_ERROR: &str = "TRANSFER_CANCELLED";

/// Default number of concurrent transfers when the setting is unset.
pub const DEFAULT_CONCURRENCY: u32 = 2;

/// Maximum number of concurrent transfers supported.
pub const MAX_CONCURRENCY: u32 = 8;

/// Clamp a requested concurrency to the supported range (1..=MAX_CONCURRENCY).
/// `None` (setting unset) yields the default.
pub fn clamp_concurrency(requested: Option<u32>) -> u32 {
    requested.unwrap_or(DEFAULT_CONCURRENCY).clamp(1, MAX_CONCURRENCY)
}

/// What a queued job does. Serialized to the frontend in camelCase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TransferKind {
    Upload,
    Download,
    UploadDir,
    DownloadDir,
}

/// Lifecycle state of a queued job. Serialized to the frontend in camelCase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TransferState {
    Queued,
    Active,
    Done,
    Failed,
    Cancelled,
}

/// Immutable snapshot of a job sent to the frontend via the `transfer-update`
/// event and `list_transfers`. Never holds the cancel token or thread state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferJobView {
    pub id: u64,
    pub profile_id: String,
    pub kind: TransferKind,
    /// Source path (local for uploads, remote for downloads).
    pub src: String,
    /// Destination path (remote for uploads, local for downloads).
    pub dst: String,
    /// Display name of the item (or current file within a folder op).
    pub filename: String,
    pub state: TransferState,
    /// Bytes transferred so far for the current file.
    pub bytes_done: u64,
    /// Total bytes for the current file; 0 when unknown (FTP / start of a folder op).
    pub bytes_total: u64,
    pub error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_concurrency_defaults_and_bounds() {
        assert_eq!(clamp_concurrency(None), DEFAULT_CONCURRENCY);
        assert_eq!(clamp_concurrency(Some(0)), 1);
        assert_eq!(clamp_concurrency(Some(1)), 1);
        assert_eq!(clamp_concurrency(Some(4)), 4);
        assert_eq!(clamp_concurrency(Some(8)), 8);
        assert_eq!(clamp_concurrency(Some(99)), 8);
    }

    #[test]
    fn kind_and_state_serialize_camel_case() {
        assert_eq!(
            serde_json::to_string(&TransferKind::UploadDir).unwrap(),
            "\"uploadDir\""
        );
        assert_eq!(
            serde_json::to_string(&TransferState::Cancelled).unwrap(),
            "\"cancelled\""
        );
    }
}
