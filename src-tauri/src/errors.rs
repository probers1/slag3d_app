use serde::{Deserialize, Serialize};

/// Structured error returned across the Tauri IPC boundary.
///
/// Commands historically returned `Result<T, String>`; that is ambiguous once
/// callers want to distinguish e.g. "not found" from "invalid path" or "archive
/// too large". This envelope serializes to
/// `{"kind": "<snake_case_kind>", "message": "<human readable>"}` so the
/// frontend can branch on `kind` without parsing free text.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppError {
    pub kind: String,
    pub message: String,
}

impl AppError {
    pub fn new(kind: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            message: message.into(),
        }
    }

    pub fn invalid_id() -> Self {
        Self::new("invalid_id", "Invalid model ID")
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new("not_found", message.into())
    }
}

impl From<String> for AppError {
    fn from(message: String) -> Self {
        Self::new("message", message)
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        Self::new("database", err.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        Self::new("io", err.to_string())
    }
}

impl From<crate::services::archive::ArchiveError> for AppError {
    fn from(err: crate::services::archive::ArchiveError) -> Self {
        Self::new("archive", format!("Archive error: {}", err))
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.kind, self.message)
    }
}

impl std::error::Error for AppError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_error_serializes_to_kind_and_message() {
        let err = AppError::new("archive", "Archive exceeds size limit");
        let value = serde_json::to_value(&err).unwrap();
        assert_eq!(value["kind"], "archive");
        assert_eq!(value["message"], "Archive exceeds size limit");

        let from_string: AppError = AppError::new("message", "boom");
        assert_eq!(from_string.kind, "message");
        assert_eq!(from_string.message, "boom");
    }
}
