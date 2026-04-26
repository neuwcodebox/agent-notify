mod common;
mod discord;
mod file_log;
mod gotify;
mod ntfy;
mod pushover;
mod slack;
mod telegram;
mod webhook;

use std::path::Path;

use reqwest::Client;
use serde::Serialize;

use crate::{
    Attachment, NotifyMessage, Result,
    config::{ChannelConfig, CheckIssue, Config},
};

use common::path_to_string;

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
    pub field: Option<String>,
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
            field: None,
            name: None,
            original_path: None,
            stored_path: None,
            mime_type: None,
            size_bytes: None,
            sha256: None,
        }
    }

    pub(super) fn sent(field: Option<String>, attachment: &Attachment) -> Self {
        Self {
            path: None,
            field,
            name: Some(attachment.name.clone()),
            original_path: Some(path_to_string(&attachment.path)),
            stored_path: None,
            mime_type: Some(attachment.mime_type.clone()),
            size_bytes: Some(attachment.size_bytes),
            sha256: Some(attachment.sha256.clone()),
        }
    }
}

pub async fn send_notification(
    channel_name: &str,
    channel: &ChannelConfig,
    message: &NotifyMessage,
) -> Result<SendResult> {
    let client = Client::new();
    match channel {
        ChannelConfig::Telegram(config) => {
            telegram::send(&client, channel_name, config, message).await
        }
        ChannelConfig::DiscordWebhook(config) => {
            discord::send_webhook(&client, channel_name, config, message).await
        }
        ChannelConfig::DiscordBot(config) => {
            discord::send_bot(&client, channel_name, config, message).await
        }
        ChannelConfig::Ntfy(config) => ntfy::send(&client, channel_name, config, message).await,
        ChannelConfig::SlackWebhook(config) => {
            slack::send_webhook(&client, channel_name, config, message).await
        }
        ChannelConfig::Pushover(config) => {
            pushover::send(&client, channel_name, config, message).await
        }
        ChannelConfig::Gotify(config) => gotify::send(&client, channel_name, config, message).await,
        ChannelConfig::Webhook(config) => {
            webhook::send(&client, channel_name, config, message).await
        }
        ChannelConfig::FileLog(config) => file_log::send(channel_name, config, message),
    }
}

pub fn check_file_log_paths(config: &Config) -> Vec<CheckIssue> {
    file_log::check_paths(config)
}
