use reqwest::Client;
use serde_json::json;

use crate::{MessageFormat, NotifyError, NotifyMessage, Priority, Result, config::GotifyConfig};

use super::{
    SendResult,
    common::{ensure_success, resolve_required_secret},
};

pub(super) async fn send(
    client: &Client,
    channel_name: &str,
    config: &GotifyConfig,
    message: &NotifyMessage,
) -> Result<SendResult> {
    if !message.attachments.is_empty() {
        return Err(NotifyError::UnsupportedAttachment {
            channel_type: "gotify".to_string(),
        });
    }

    let token = resolve_required_secret(channel_name, "token", &config.token, &config.token_env)?;
    let url = format!("{}/message", config.server.trim_end_matches('/'));
    let payload = gotify_payload(message, config);

    let response = client
        .post(url)
        .query(&[("token", token)])
        .json(&payload)
        .send()
        .await?;
    ensure_success("gotify", response.status(), response.text().await?)?;

    Ok(SendResult {
        id: message.id.clone(),
        attachments: Vec::new(),
    })
}

fn gotify_payload(message: &NotifyMessage, config: &GotifyConfig) -> serde_json::Value {
    let mut payload = json!({
        "title": message.title,
        "message": message.body.as_deref().unwrap_or(&message.title),
        "priority": config.priority.unwrap_or_else(|| gotify_priority(message.priority)),
    });
    if message.format == MessageFormat::Markdown {
        payload["extras"] = json!({
            "client::display": {
                "contentType": "text/markdown"
            }
        });
    }
    payload
}

fn gotify_priority(priority: Priority) -> i64 {
    match priority {
        Priority::Info | Priority::Success => 5,
        Priority::Warning => 6,
        Priority::Error => 8,
        Priority::Critical => 10,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use reqwest::Client;
    use serde_json::json;
    use tempfile::tempdir;

    use crate::{Attachment, MessageFormat};

    use super::*;

    #[test]
    fn gotify_payload_includes_message_fields() {
        let message = NotifyMessage::new(
            "Done".to_string(),
            Some("**Completed.**".to_string()),
            MessageFormat::Markdown,
            Priority::Critical,
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let config = GotifyConfig {
            server: "https://gotify.example.com".to_string(),
            token: Some("app-token".to_string()),
            token_env: None,
            priority: None,
        };

        let payload = gotify_payload(&message, &config);

        assert_eq!(payload["title"], json!("Done"));
        assert_eq!(payload["message"], json!("**Completed.**"));
        assert_eq!(payload["priority"], json!(10));
        assert_eq!(
            payload["extras"]["client::display"]["contentType"],
            json!("text/markdown")
        );
    }

    #[test]
    fn gotify_config_priority_overrides_message_priority() {
        let message = NotifyMessage::new(
            "Done".to_string(),
            Some("Completed.".to_string()),
            MessageFormat::Text,
            Priority::Critical,
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let config = GotifyConfig {
            server: "https://gotify.example.com".to_string(),
            token: Some("app-token".to_string()),
            token_env: None,
            priority: Some(3),
        };

        let payload = gotify_payload(&message, &config);

        assert_eq!(payload["priority"], json!(3));
    }

    #[tokio::test]
    async fn gotify_rejects_attachments() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("report.txt");
        fs::write(&source, "hello").unwrap();
        let message = NotifyMessage::new(
            "Done".to_string(),
            Some("Attached.".to_string()),
            MessageFormat::Text,
            Priority::Info,
            Vec::new(),
            vec![Attachment::from_path(&source).unwrap()],
        )
        .unwrap();
        let config = GotifyConfig {
            server: "https://gotify.example.com".to_string(),
            token: Some("app-token".to_string()),
            token_env: None,
            priority: None,
        };

        let error = send(&Client::new(), "phone", &config, &message)
            .await
            .unwrap_err();

        assert_eq!(error.code(), "UNSUPPORTED_ATTACHMENT");
    }

    #[tokio::test]
    async fn gotify_http_errors_do_not_leak_token_url() {
        let message = NotifyMessage::new(
            "Done".to_string(),
            Some("Completed.".to_string()),
            MessageFormat::Text,
            Priority::Info,
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let config = GotifyConfig {
            server: "http://127.0.0.1:1/secret-path".to_string(),
            token: Some("secret-token".to_string()),
            token_env: None,
            priority: None,
        };

        let error = send(&Client::new(), "phone", &config, &message)
            .await
            .unwrap_err();
        let error = error.to_string();

        assert!(!error.contains("secret-token"));
        assert!(!error.contains("secret-path"));
    }
}
