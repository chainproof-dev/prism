//! Engine-wide error strategy (docs/06-RUST-CORE.md § 10). `anyhow` is banned
//! here; every public API returns `Result<T, EngineError>`.

/// The one error enum for the engine. Mirrors the TS `IpcError` kinds 1:1
/// (PRISM-IPC-003).
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// I/O failure with the failing path.
    #[error("io: {path:?}: {source}")]
    Io {
        /// Display path (already redacted per policy at the log layer).
        path: String,
        /// Underlying error.
        source: std::io::Error,
    },
    /// OS/Win32 failure code.
    #[error("os: {code:#x} {msg}")]
    Os {
        /// Raw OS error code.
        code: i32,
        /// Static description.
        msg: &'static str,
    },
    /// Scan-lifecycle failure.
    #[error("scan {id}: {stage}: {msg}")]
    Scan {
        /// Scan lease id.
        id: u32,
        /// Stage where it failed.
        stage: &'static str,
        /// Message.
        msg: String,
    },
    /// Entitlement verification failed (premium commands).
    #[error("entitlement: {0}")]
    Entitlement(#[from] prism_types::licensing::EntitlementError),
    /// NTFS parse failure.
    #[error("ntfs: {0}")]
    Ntfs(#[from] prism_ntfs::Error),
    /// Invalid argument/field at the command boundary.
    #[error("invalid: {field}")]
    Invalid {
        /// Which field.
        field: &'static str,
    },
    /// Engine busy (single active scan per lease class).
    #[error("busy: {op}")]
    Busy {
        /// Operation.
        op: &'static str,
    },
    /// Cancelled by user.
    #[error("cancelled")]
    Cancelled,
    /// Internal invariant broke (catch_unwind boundary only).
    #[error("internal: {0}")]
    Internal(String),
}

impl EngineError {
    /// Map to the TS wire kind (PRISM-IPC-003 total mapping).
    pub fn wire_kind(&self) -> &'static str {
        match self {
            Self::Io { .. } => "io",
            Self::Os { .. } => "io",
            Self::Scan { .. } => "engine",
            Self::Entitlement(_) => "entitlement",
            Self::Ntfs(_) => "engine",
            Self::Invalid { .. } => "invalid-args",
            Self::Busy { .. } => "busy",
            Self::Cancelled => "cancelled",
            Self::Internal(_) => "engine",
        }
    }
}

/// Result alias for the engine.
pub type Result<T> = std::result::Result<T, EngineError>;
