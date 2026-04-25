use std::{path::PathBuf, time::Duration};

use reqwest::{Client, StatusCode, multipart};
use serde::{Deserialize, Serialize};

use crate::{MessageFormat, NotifyError, NotifyMessage, Priority, Result, config::WebhookConfig};

use super::{
    SendResult, StoredAttachment,
    common::{
        multipart_part, path_to_string, resolve_optional_secret, resolve_required_secret,
        trim_response_body,
    },
};

const DEFAULT_WEBHOOK_TIMEOUT_SECONDS: u64 = 15;

pub(super) async fn send(
    client: &Client,
    channel_name: &str,
    config: &WebhookConfig,
    message: &NotifyMessage,
) -> Result<SendResult> {
    let url = resolve_required_secret(channel_name, "url", &config.url, &config.url_env)?;
    let auth_header = resolve_optional_secret(
        channel_name,
        "auth_header",
        &config.auth_header,
        &config.auth_header_env,
    )?;
    let payload = WebhookPayload::from_message(message)?;
    let timeout = Duration::from_secs(
        config
            .timeout_seconds
            .unwrap_or(DEFAULT_WEBHOOK_TIMEOUT_SECONDS),
    );

    let mut request = if message.attachments.is_empty() {
        client.post(&url).json(&payload)
    } else {
        let payload_json = serde_json::to_string(&payload)?;
        let mut form = multipart::Form::new().part(
            "payload",
            multipart::Part::text(payload_json).mime_str("application/json")?,
        );
        for (index, attachment) in message.attachments.iter().enumerate() {
            form = form.part(
                format!("file{index}"),
                multipart_part(attachment, &attachment.name)?,
            );
        }
        client.post(&url).multipart(form)
    }
    .timeout(timeout);

    if let Some(auth_header) = auth_header {
        request = request.header(reqwest::header::AUTHORIZATION, auth_header);
    }

    let response = request.send().await?;
    let id = handle_response(response.status(), response.text().await?, &message.id)?;

    Ok(SendResult {
        id,
        attachments: message
            .attachments
            .iter()
            .enumerate()
            .map(|(index, attachment)| {
                StoredAttachment::sent(Some(format!("file{index}")), attachment)
            })
            .collect(),
    })
}

#[derive(Debug, Serialize)]
struct WebhookPayload<'a> {
    version: &'static str,
    id: &'a str,
    #[serde(with = "time::serde::rfc3339")]
    timestamp: time::OffsetDateTime,
    source: WebhookSource,
    message: WebhookMessage<'a>,
    attachments: Vec<WebhookAttachment<'a>>,
}

#[derive(Debug, Serialize)]
struct WebhookSource {
    app: &'static str,
    hostname: String,
    cwd: String,
}

#[derive(Debug, Serialize)]
struct WebhookMessage<'a> {
    title: &'a str,
    body: Option<&'a str>,
    format: MessageFormat,
    priority: Priority,
    tags: &'a [String],
}

#[derive(Debug, Serialize)]
struct WebhookAttachment<'a> {
    field: String,
    name: &'a str,
    mime_type: &'a str,
    size_bytes: u64,
    sha256: &'a str,
}

impl<'a> WebhookPayload<'a> {
    fn from_message(message: &'a NotifyMessage) -> Result<Self> {
        Ok(Self {
            version: "1",
            id: &message.id,
            timestamp: message.timestamp,
            source: WebhookSource {
                app: "agent-notify",
                hostname: hostname::get()
                    .map(|value| value.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "unknown".to_string()),
                cwd: std::env::current_dir()
                    .map(|value| path_to_string(&value))
                    .map_err(|source| NotifyError::Io {
                        path: PathBuf::from("."),
                        source,
                    })?,
            },
            message: WebhookMessage {
                title: &message.title,
                body: message.body.as_deref(),
                format: message.format,
                priority: message.priority,
                tags: &message.tags,
            },
            attachments: message
                .attachments
                .iter()
                .enumerate()
                .map(|(index, attachment)| WebhookAttachment {
                    field: format!("file{index}"),
                    name: &attachment.name,
                    mime_type: &attachment.mime_type,
                    size_bytes: attachment.size_bytes,
                    sha256: &attachment.sha256,
                })
                .collect(),
        })
    }
}

#[derive(Debug, Deserialize)]
struct WebhookResponse {
    ok: bool,
    id: Option<String>,
    error: Option<WebhookResponseError>,
}

#[derive(Debug, Deserialize)]
struct WebhookResponseError {
    code: Option<String>,
    message: String,
}

fn handle_response(status: StatusCode, body: String, fallback_id: &str) -> Result<String> {
    if !status.is_success() {
        return Err(NotifyError::Provider(format!(
            "webhook returned HTTP {status}: {}",
            trim_response_body(&body)
        )));
    }

    match serde_json::from_str::<WebhookResponse>(&body) {
        Ok(response) if response.ok => Ok(response.id.unwrap_or_else(|| fallback_id.to_string())),
        Ok(response) => {
            let message = response
                .error
                .map(|error| match error.code {
                    Some(code) => format!("{code}: {}", error.message),
                    None => error.message,
                })
                .unwrap_or_else(|| "webhook returned ok=false".to_string());
            Err(NotifyError::Provider(message))
        }
        Err(_) => Ok(fallback_id.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use reqwest::StatusCode;
    use serde_json::json;
    use tempfile::tempdir;

    use crate::{Attachment, MessageFormat};

    use super::*;

    #[test]
    fn webhook_payload_includes_protocol_fields() {
        let message = NotifyMessage::new(
            "Task completed".to_string(),
            Some("Done.".to_string()),
            MessageFormat::Text,
            Priority::Success,
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let payload = WebhookPayload::from_message(&message).unwrap();
        let payload_json = serde_json::to_value(payload).unwrap();

        assert_eq!(payload_json["version"], json!("1"));
        assert_eq!(payload_json["source"]["app"], json!("agent-notify"));
        assert_eq!(payload_json["message"]["priority"], json!("success"));
        assert_eq!(payload_json["attachments"], json!([]));
    }

    #[test]
    fn webhook_payload_includes_attachment_fields() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("report.txt");
        fs::write(&source, "hello").unwrap();
        let attachment = Attachment::from_path(&source).unwrap();
        let message = NotifyMessage::new(
            "Done".to_string(),
            Some("Attached.".to_string()),
            MessageFormat::Text,
            Priority::Info,
            vec!["report".to_string()],
            vec![attachment],
        )
        .unwrap();
        let payload = WebhookPayload::from_message(&message).unwrap();
        let payload_json = serde_json::to_value(payload).unwrap();

        assert_eq!(payload_json["attachments"][0]["field"], json!("file0"));
        assert_eq!(payload_json["attachments"][0]["name"], json!("report.txt"));
        assert_eq!(
            payload_json["attachments"][0]["mime_type"],
            json!("text/plain")
        );
    }

    #[test]
    fn webhook_response_ok_false_is_failure() {
        let error = handle_response(
            StatusCode::OK,
            r#"{"ok":false,"error":{"code":"INVALID_PAYLOAD","message":"bad"}}"#.to_string(),
            "fallback",
        )
        .unwrap_err();

        assert!(error.to_string().contains("INVALID_PAYLOAD"));
    }

    #[test]
    fn webhook_response_non_json_success_is_accepted() {
        let id = handle_response(StatusCode::OK, "accepted".to_string(), "fallback").unwrap();

        assert_eq!(id, "fallback");
    }
}
