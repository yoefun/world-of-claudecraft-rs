#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("hash mismatch for {path}")]
    HashMismatch { path: String },
    #[error("bad signature")]
    Signature,
    #[error("target mismatch")]
    TargetMismatch,
    #[error("delta: {0}")]
    Delta(String),
    #[error("{0}")]
    Msg(String),
}
