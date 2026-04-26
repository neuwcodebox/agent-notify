use reqwest::Client;
use serde_json::json;

use crate::{NotifyError, NotifyMessage, Result, config::SlackWebhookConfig};

use super::{
    SendResult,
    common::{ensure_success, render_message, resolve_required_secret},
};

pub(super) async fn send_webhook(
    client: &Client,
    channel_name: &str,
    config: &SlackWebhookConfig,
    message: &NotifyMessage,
) -> Result<SendResult> {
    if !message.attachments.is_empty() {
        return Err(NotifyError::UnsupportedAttachment {
            channel_type: "slack-webhook".to_string(),
        });
    }

    let url = resolve_required_secret(
        channel_name,
        "webhook_url",
        &config.webhook_url,
        &config.webhook_url_env,
    )?;
    let payload = slack_payload(message, config);

    let response = client.post(&url).json(&payload).send().await?;
    ensure_success("slack-webhook", response.status(), response.text().await?)?;

    Ok(SendResult {
        id: message.id.clone(),
        attachments: Vec::new(),
    })
}

fn slack_payload(message: &NotifyMessage, config: &SlackWebhookConfig) -> serde_json::Value {
    let text = if config.allow_mentions.unwrap_or(false) {
        render_message(message)
    } else {
        neutralize_mass_mentions(&render_message(message))
    };
    let mut payload = json!({ "text": text });
    if let Some(username) = config.username.as_deref() {
        payload["username"] = json!(username);
    }
    if let Some(icon_emoji) = config.icon_emoji.as_deref() {
        payload["icon_emoji"] = json!(icon_emoji);
    }
    if let Some(icon_url) = config.icon_url.as_deref() {
        payload["icon_url"] = json!(icon_url);
    }
    payload
}

fn neutralize_mass_mentions(text: &str) -> String {
    text.replace("@everyone", "@ everyone")
        .replace("@channel", "@ channel")
        .replace("@here", "@ here")
        .replace("<!everyone>", "<! everyone>")
        .replace("<!channel>", "<! channel>")
        .replace("<!here>", "<! here>")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use reqwest::Client;
    use serde_json::json;
    use tempfile::tempdir;

    use crate::{Attachment, MessageFormat, Priority};

    use super::*;

    #[test]
    fn slack_payload_disables_mass_mentions_by_default() {
        let message = NotifyMessage::new(
            "Done".to_string(),
            Some("@channel complete".to_string()),
            MessageFormat::Text,
            Priority::Success,
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let config = SlackWebhookConfig {
            webhook_url: Some("https://hooks.slack.com/services/test".to_string()),
            webhook_url_env: None,
            username: Some("Agent Notify".to_string()),
            icon_emoji: Some(":robot_face:".to_string()),
            icon_url: None,
            allow_mentions: None,
        };

        let payload = slack_payload(&message, &config);

        assert_eq!(payload["username"], json!("Agent Notify"));
        assert_eq!(payload["icon_emoji"], json!(":robot_face:"));
        assert!(payload["text"].as_str().unwrap().contains("[success] Done"));
        assert!(payload["text"].as_str().unwrap().contains("@ channel"));
        assert!(!payload["text"].as_str().unwrap().contains("@channel"));
    }

    #[tokio::test]
    async fn slack_rejects_attachments() {
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
        let config = SlackWebhookConfig {
            webhook_url: Some("https://hooks.slack.com/services/test".to_string()),
            webhook_url_env: None,
            username: None,
            icon_emoji: None,
            icon_url: None,
            allow_mentions: None,
        };

        let error = send_webhook(&Client::new(), "team", &config, &message)
            .await
            .unwrap_err();

        assert_eq!(error.code(), "UNSUPPORTED_ATTACHMENT");
    }
}
