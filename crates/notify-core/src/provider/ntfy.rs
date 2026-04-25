use reqwest::Client;

use crate::{NotifyError, NotifyMessage, Priority, Result, config::NtfyConfig};

use super::{
    SendResult,
    common::{ensure_success, resolve_optional_secret, resolve_required_secret},
};

const DEFAULT_NTFY_SERVER: &str = "https://ntfy.sh";

pub(super) async fn send(
    client: &Client,
    channel_name: &str,
    config: &NtfyConfig,
    message: &NotifyMessage,
) -> Result<SendResult> {
    if !message.attachments.is_empty() {
        return Err(NotifyError::UnsupportedAttachment {
            channel_type: "ntfy".to_string(),
        });
    }

    let topic = resolve_required_secret(channel_name, "topic", &config.topic, &config.topic_env)?;
    let token = resolve_optional_secret(channel_name, "token", &config.token, &config.token_env)?;
    let server = config.server.as_deref().unwrap_or(DEFAULT_NTFY_SERVER);
    let url = format!(
        "{}/{}",
        server.trim_end_matches('/'),
        topic.trim_start_matches('/')
    );

    let mut request = client
        .post(url)
        .header("Title", &message.title)
        .header("Priority", ntfy_priority(message.priority))
        .body(message.body.clone().unwrap_or_default());
    if !message.tags.is_empty() {
        request = request.header("Tags", message.tags.join(","));
    }
    if let Some(token) = token {
        request = request.header(reqwest::header::AUTHORIZATION, format!("Bearer {token}"));
    }

    let response = request.send().await?;
    ensure_success("ntfy", response.status(), response.text().await?)?;

    Ok(SendResult {
        id: message.id.clone(),
        attachments: Vec::new(),
    })
}

fn ntfy_priority(priority: Priority) -> &'static str {
    match priority {
        Priority::Info | Priority::Success => "default",
        Priority::Warning | Priority::Error => "high",
        Priority::Critical => "urgent",
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use reqwest::Client;
    use tempfile::tempdir;

    use crate::{Attachment, MessageFormat};

    use super::*;

    #[tokio::test]
    async fn ntfy_rejects_attachments() {
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
        let config = NtfyConfig {
            server: None,
            topic: Some("topic".to_string()),
            topic_env: None,
            token: None,
            token_env: None,
        };

        let error = send(&Client::new(), "phone", &config, &message)
            .await
            .unwrap_err();

        assert_eq!(error.code(), "UNSUPPORTED_ATTACHMENT");
    }

    #[test]
    fn ntfy_priority_maps_levels() {
        assert_eq!(ntfy_priority(Priority::Info), "default");
        assert_eq!(ntfy_priority(Priority::Success), "default");
        assert_eq!(ntfy_priority(Priority::Warning), "high");
        assert_eq!(ntfy_priority(Priority::Error), "high");
        assert_eq!(ntfy_priority(Priority::Critical), "urgent");
    }
}
