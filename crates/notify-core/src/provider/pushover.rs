use reqwest::Client;

use crate::{NotifyError, NotifyMessage, Priority, Result, config::PushoverConfig};

use super::{
    SendResult,
    common::{ensure_success, resolve_required_secret},
};

const PUSHOVER_MESSAGES_URL: &str = "https://api.pushover.net/1/messages.json";

pub(super) async fn send(
    client: &Client,
    channel_name: &str,
    config: &PushoverConfig,
    message: &NotifyMessage,
) -> Result<SendResult> {
    if !message.attachments.is_empty() {
        return Err(NotifyError::UnsupportedAttachment {
            channel_type: "pushover".to_string(),
        });
    }

    let token = resolve_required_secret(channel_name, "token", &config.token, &config.token_env)?;
    let user = resolve_required_secret(channel_name, "user", &config.user, &config.user_env)?;
    let form = pushover_form(message, &token, &user, config);

    let response = client
        .post(PUSHOVER_MESSAGES_URL)
        .form(&form)
        .send()
        .await?;
    ensure_success("pushover", response.status(), response.text().await?)?;

    Ok(SendResult {
        id: message.id.clone(),
        attachments: Vec::new(),
    })
}

fn pushover_form(
    message: &NotifyMessage,
    token: &str,
    user: &str,
    config: &PushoverConfig,
) -> Vec<(String, String)> {
    let mut form = vec![
        ("token".to_string(), token.to_string()),
        ("user".to_string(), user.to_string()),
        ("title".to_string(), message.title.clone()),
        (
            "message".to_string(),
            message
                .body
                .clone()
                .unwrap_or_else(|| message.title.clone()),
        ),
        (
            "priority".to_string(),
            pushover_priority(message.priority).to_string(),
        ),
    ];
    if let Some(device) = config.device.as_deref() {
        form.push(("device".to_string(), device.to_string()));
    }
    if let Some(sound) = config.sound.as_deref() {
        form.push(("sound".to_string(), sound.to_string()));
    }
    form
}

fn pushover_priority(priority: Priority) -> i8 {
    match priority {
        Priority::Info | Priority::Success | Priority::Warning => 0,
        Priority::Error | Priority::Critical => 1,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use reqwest::Client;
    use tempfile::tempdir;

    use crate::{Attachment, MessageFormat};

    use super::*;

    #[test]
    fn pushover_form_includes_required_fields() {
        let message = NotifyMessage::new(
            "Done".to_string(),
            Some("Completed.".to_string()),
            MessageFormat::Text,
            Priority::Error,
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let config = PushoverConfig {
            token: Some("app-token".to_string()),
            token_env: None,
            user: Some("user-key".to_string()),
            user_env: None,
            device: Some("phone".to_string()),
            sound: Some("pushover".to_string()),
        };
        let form = pushover_form(&message, "app-token", "user-key", &config);

        assert!(form.contains(&("token".to_string(), "app-token".to_string())));
        assert!(form.contains(&("user".to_string(), "user-key".to_string())));
        assert!(form.contains(&("title".to_string(), "Done".to_string())));
        assert!(form.contains(&("message".to_string(), "Completed.".to_string())));
        assert!(form.contains(&("priority".to_string(), "1".to_string())));
        assert!(form.contains(&("device".to_string(), "phone".to_string())));
        assert!(form.contains(&("sound".to_string(), "pushover".to_string())));
    }

    #[test]
    fn pushover_priority_maps_levels_without_emergency_receipts() {
        assert_eq!(pushover_priority(Priority::Info), 0);
        assert_eq!(pushover_priority(Priority::Success), 0);
        assert_eq!(pushover_priority(Priority::Warning), 0);
        assert_eq!(pushover_priority(Priority::Error), 1);
        assert_eq!(pushover_priority(Priority::Critical), 1);
    }

    #[tokio::test]
    async fn pushover_rejects_attachments() {
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
        let config = PushoverConfig {
            token: Some("app-token".to_string()),
            token_env: None,
            user: Some("user-key".to_string()),
            user_env: None,
            device: None,
            sound: None,
        };

        let error = send(&Client::new(), "phone", &config, &message)
            .await
            .unwrap_err();

        assert_eq!(error.code(), "UNSUPPORTED_ATTACHMENT");
    }
}
