use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

use serde::Serialize;
use time::{format_description::FormatItem, macros::format_description};

use crate::{
    NotifyError, Result,
    config::{ChannelConfig, CheckIssue, Config, FileLogConfig},
    message::NotifyMessage,
};

const MONTH_FORMAT: &[FormatItem<'_>] = format_description!("[year]-[month]");

#[derive(Debug, Clone)]
pub struct SendResult {
    pub id: String,
    pub attachments: Vec<StoredAttachment>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StoredAttachment {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stored_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

impl StoredAttachment {
    pub fn dry_run(path: &Path) -> Self {
        Self {
            path: Some(path_to_string(path)),
            name: None,
            original_path: None,
            stored_path: None,
            mime_type: None,
            size_bytes: None,
            sha256: None,
        }
    }
}

pub fn send_notification(
    channel_name: &str,
    channel: &ChannelConfig,
    message: &NotifyMessage,
) -> Result<SendResult> {
    match channel {
        ChannelConfig::FileLog(config) => send_file_log(channel_name, config, message),
        other => Err(NotifyError::UnsupportedProvider(
            other.type_name().to_string(),
        )),
    }
}

pub fn check_file_log_paths(config: &Config) -> Vec<CheckIssue> {
    config
        .channels
        .iter()
        .filter_map(|(name, channel)| match channel {
            ChannelConfig::FileLog(file_log) => Some((name, file_log)),
            _ => None,
        })
        .filter_map(|(name, file_log)| {
            match fs::metadata(&file_log.path) {
                Ok(metadata) if metadata.is_file() => {
                    return Some(CheckIssue::error(
                        Some(name),
                        "FILE_LOG_PATH_INVALID",
                        format!(
                            "channel \"{name}\" file-log path {} is a file",
                            file_log.path.display()
                        ),
                    ));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Some(CheckIssue::error(
                        Some(name),
                        "FILE_LOG_PATH_INVALID",
                        format!(
                            "channel \"{name}\" file-log path {} cannot be checked: {error}",
                            file_log.path.display()
                        ),
                    ));
                }
            }

            fs::create_dir_all(&file_log.path).err().map(|error| {
                CheckIssue::error(
                    Some(name),
                    "FILE_LOG_PATH_CREATE_FAILED",
                    format!(
                        "channel \"{name}\" file-log path {} cannot be created: {error}",
                        file_log.path.display()
                    ),
                )
            })
        })
        .collect()
}

fn send_file_log(
    channel_name: &str,
    config: &FileLogConfig,
    message: &NotifyMessage,
) -> Result<SendResult> {
    fs::create_dir_all(&config.path).map_err(|source| NotifyError::Io {
        path: config.path.clone(),
        source,
    })?;

    let month = message
        .timestamp
        .format(MONTH_FORMAT)
        .map_err(|error| NotifyError::Validation(error.to_string()))?;
    let attachment_root = config
        .path
        .join("attachments")
        .join(&month)
        .join(&message.id);
    fs::create_dir_all(&attachment_root).map_err(|source| NotifyError::Io {
        path: attachment_root.clone(),
        source,
    })?;

    let mut stored_attachments = Vec::new();
    for attachment in &message.attachments {
        let stored_name = unique_file_name(&attachment_root, &attachment.name);
        let destination = attachment_root.join(&stored_name);
        fs::copy(&attachment.path, &destination).map_err(|source| NotifyError::Io {
            path: destination.clone(),
            source,
        })?;

        stored_attachments.push(StoredAttachment {
            path: None,
            name: Some(attachment.name.clone()),
            original_path: Some(path_to_string(&attachment.path)),
            stored_path: Some(path_to_string(
                Path::new("attachments")
                    .join(&month)
                    .join(&message.id)
                    .join(&stored_name)
                    .as_path(),
            )),
            mime_type: Some(attachment.mime_type.clone()),
            size_bytes: Some(attachment.size_bytes),
            sha256: Some(attachment.sha256.clone()),
        });
    }

    let record = FileLogRecord {
        version: "1",
        id: &message.id,
        timestamp: message.timestamp,
        channel: channel_name,
        channel_type: "file-log",
        message: FileLogMessage {
            title: &message.title,
            body: message.body.as_deref(),
            format: message.format,
            priority: message.priority,
            tags: &message.tags,
        },
        attachments: &stored_attachments,
    };

    let json = serde_json::to_string(&record)?;
    let log_path = config.path.join("notifications.jsonl");
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|source| NotifyError::Io {
            path: log_path.clone(),
            source,
        })?;
    writeln!(log, "{json}").map_err(|source| NotifyError::Io {
        path: log_path,
        source,
    })?;

    Ok(SendResult {
        id: message.id.clone(),
        attachments: stored_attachments,
    })
}

#[derive(Debug, Serialize)]
struct FileLogRecord<'a> {
    version: &'static str,
    id: &'a str,
    #[serde(with = "time::serde::rfc3339")]
    timestamp: time::OffsetDateTime,
    channel: &'a str,
    #[serde(rename = "type")]
    channel_type: &'static str,
    message: FileLogMessage<'a>,
    attachments: &'a [StoredAttachment],
}

#[derive(Debug, Serialize)]
struct FileLogMessage<'a> {
    title: &'a str,
    body: Option<&'a str>,
    format: crate::MessageFormat,
    priority: crate::Priority,
    tags: &'a [String],
}

fn unique_file_name(directory: &Path, file_name: &str) -> String {
    let candidate = directory.join(file_name);
    if !candidate.exists() {
        return file_name.to_string();
    }

    let path = Path::new(file_name);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(file_name);
    let extension = path.extension().and_then(|value| value.to_str());

    for index in 1.. {
        let name = match extension {
            Some(extension) => format!("{stem}-{index}.{extension}"),
            None => format!("{stem}-{index}"),
        };
        if !directory.join(&name).exists() {
            return name;
        }
    }

    unreachable!("unbounded suffix search should always return")
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use crate::{Attachment, MessageFormat, Priority};

    use super::*;

    #[test]
    fn file_log_writes_jsonl_and_copies_attachment() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("report.txt");
        fs::write(&source, "hello").unwrap();
        let message = NotifyMessage::new(
            "Done".to_string(),
            Some("Attached.".to_string()),
            MessageFormat::Text,
            Priority::Info,
            vec!["report".to_string()],
            vec![Attachment::from_path(&source).unwrap()],
        )
        .unwrap();
        let config = FileLogConfig {
            path: dir.path().join("notify-log"),
        };

        let result = send_file_log("local", &config, &message).unwrap();

        let jsonl = fs::read_to_string(config.path.join("notifications.jsonl")).unwrap();
        assert!(jsonl.contains("\"channel\":\"local\""));
        assert_eq!(result.attachments.len(), 1);
        let stored_path = result.attachments[0].stored_path.as_ref().unwrap();
        assert!(config.path.join(stored_path).exists());
    }

    #[test]
    fn file_log_renames_colliding_attachments() {
        let dir = tempdir().unwrap();
        let source_a = dir.path().join("a").join("chart.png");
        let source_b = dir.path().join("b").join("chart.png");
        fs::create_dir_all(source_a.parent().unwrap()).unwrap();
        fs::create_dir_all(source_b.parent().unwrap()).unwrap();
        fs::write(&source_a, "a").unwrap();
        fs::write(&source_b, "b").unwrap();
        let message = NotifyMessage::new(
            "Charts".to_string(),
            Some("Attached.".to_string()),
            MessageFormat::Text,
            Priority::Info,
            Vec::new(),
            vec![
                Attachment::from_path(&source_a).unwrap(),
                Attachment::from_path(&source_b).unwrap(),
            ],
        )
        .unwrap();
        let config = FileLogConfig {
            path: dir.path().join("notify-log"),
        };

        let result = send_file_log("local", &config, &message).unwrap();

        let paths = result
            .attachments
            .iter()
            .map(|attachment| attachment.stored_path.as_deref().unwrap())
            .collect::<Vec<_>>();
        assert!(paths.iter().any(|path| path.ends_with("chart.png")));
        assert!(paths.iter().any(|path| path.ends_with("chart-1.png")));
    }
}
