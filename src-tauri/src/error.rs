//! Structured errors for the IPC boundary. Serialized as
//! `{ "kind": "...", "detail": "..." }` so the frontend can branch on kind
//! instead of string-matching messages.
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(tag = "kind", content = "detail", rename_all = "camelCase")]
pub enum CmdError {
    #[error("canceled")]
    Canceled,
    #[error("not found: {0}")]
    NotFound(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("{0}")]
    Internal(String),
}

impl From<std::io::Error> for CmdError {
    fn from(e: std::io::Error) -> Self {
        use std::io::ErrorKind::*;
        match e.kind() {
            NotFound => Self::NotFound(e.to_string()),
            PermissionDenied => Self::PermissionDenied(e.to_string()),
            _ => Self::Io(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_kind_and_detail() {
        let j = serde_json::to_value(CmdError::Canceled).unwrap();
        assert_eq!(j, serde_json::json!({ "kind": "canceled" }));
        let j = serde_json::to_value(CmdError::Io("disk full".into())).unwrap();
        assert_eq!(
            j,
            serde_json::json!({ "kind": "io", "detail": "disk full" })
        );
    }

    #[test]
    fn io_error_kind_maps_to_variant() {
        let nf = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
        assert!(matches!(CmdError::from(nf), CmdError::NotFound(_)));
        let pd = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "nope");
        assert!(matches!(CmdError::from(pd), CmdError::PermissionDenied(_)));
    }
}
