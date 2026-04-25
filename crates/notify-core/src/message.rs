use std::{
    fmt,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    str::FromStr,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use ulid::Ulid;

use crate::{NotifyError, Result};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    #[default]
    Info,
    Success,
    Warning,
    Error,
    Critical,
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Critical => "critical",
        };
        f.write_str(value)
    }
}

impl FromStr for Priority {
    type Err = NotifyError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "info" => Ok(Self::Info),
            "success" => Ok(Self::Success),
            "warning" => Ok(Self::Warning),
            "error" => Ok(Self::Error),
            "critical" => Ok(Self::Critical),
            _ => Err(NotifyError::InvalidInput(format!(
                "invalid priority \"{value}\""
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageFormat {
    #[default]
    Text,
    Markdown,
}

impl fmt::Display for MessageFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Text => "text",
            Self::Markdown => "markdown",
        };
        f.write_str(value)
    }
}

impl FromStr for MessageFormat {
    type Err = NotifyError;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "text" => Ok(Self::Text),
            "markdown" => Ok(Self::Markdown),
            _ => Err(NotifyError::InvalidInput(format!(
                "invalid format \"{value}\""
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct NotifyMessage {
    pub id: String,
    #[serde(with = "time::serde::rfc3339")]
    pub timestamp: OffsetDateTime,
    pub title: String,
    pub body: Option<String>,
    pub format: MessageFormat,
    pub priority: Priority,
    pub tags: Vec<String>,
    pub attachments: Vec<Attachment>,
}

impl NotifyMessage {
    pub fn new(
        title: String,
        body: Option<String>,
        format: MessageFormat,
        priority: Priority,
        tags: Vec<String>,
        attachments: Vec<Attachment>,
    ) -> Result<Self> {
        if title.trim().is_empty() {
            return Err(NotifyError::InvalidInput("title required".to_string()));
        }
        if body.as_deref().unwrap_or("").is_empty() && attachments.is_empty() {
            return Err(NotifyError::InvalidInput(
                "body or file required".to_string(),
            ));
        }

        Ok(Self {
            id: Ulid::new().to_string(),
            timestamp: OffsetDateTime::now_utc(),
            title,
            body,
            format,
            priority,
            tags,
            attachments,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Attachment {
    pub path: PathBuf,
    pub name: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub sha256: String,
}

impl Attachment {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let metadata = std::fs::metadata(path).map_err(|source| NotifyError::Io {
            path: path.to_path_buf(),
            source,
        })?;

        if metadata.is_dir() {
            return Err(NotifyError::InvalidInput(format!(
                "{} is a directory, not a file",
                path.display()
            )));
        }

        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| {
                NotifyError::InvalidInput(format!("invalid attachment path {}", path.display()))
            })?
            .to_string();
        let mime_type = mime_guess::from_path(path)
            .first_or_octet_stream()
            .essence_str()
            .to_string();
        let sha256 = sha256_file(path)?;

        Ok(Self {
            path: path.to_path_buf(),
            name,
            mime_type,
            size_bytes: metadata.len(),
            sha256,
        })
    }
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).map_err(|source| NotifyError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];

    loop {
        let read = file.read(&mut buffer).map_err(|source| NotifyError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn message_rejects_blank_title() {
        let error = NotifyMessage::new(
            "   ".to_string(),
            Some("body".to_string()),
            MessageFormat::Text,
            Priority::Info,
            Vec::new(),
            Vec::new(),
        )
        .unwrap_err();

        assert_eq!(error.code(), "INVALID_INPUT");
        assert_eq!(error.to_string(), "title required");
    }

    #[test]
    fn attachment_from_path_records_metadata() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("report.txt");
        fs::write(&path, "hello").unwrap();

        let attachment = Attachment::from_path(&path).unwrap();

        assert_eq!(attachment.name, "report.txt");
        assert_eq!(attachment.mime_type, "text/plain");
        assert_eq!(attachment.size_bytes, 5);
        assert_eq!(
            attachment.sha256,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn attachment_rejects_directories() {
        let dir = tempdir().unwrap();

        let error = Attachment::from_path(dir.path()).unwrap_err();

        assert_eq!(error.code(), "INVALID_INPUT");
        assert!(error.to_string().contains("is a directory"));
    }
}
