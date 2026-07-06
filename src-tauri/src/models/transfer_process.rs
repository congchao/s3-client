use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TransferStatus {
    Waiting,
    Uploading,
    Downloading,
    Completed,
    Failed,
    Cancelled,
}

// 传输进度结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferProgress {
    pub id: String,
    pub config_id: String,
    pub name: String,
    pub from_path: String,
    pub to_path: String,
    pub size: u64,
    pub progress: f64, // 0.0 to 100.0
    pub status: TransferStatus,
}
