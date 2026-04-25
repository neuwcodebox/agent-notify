use reqwest::{Client, multipart};
use serde_json::json;

use crate::{
    Attachment, NotifyMessage, Result,
    config::{DiscordBotConfig, DiscordWebhookConfig},
};

use super::{
    SendResult, StoredAttachment,
    common::{ensure_success, multipart_part, render_message, resolve_required_secret},
};

pub(super) async fn send_webhook(
    client: &Client,
    channel_name: &str,
    config: &DiscordWebhookConfig,
    message: &NotifyMessage,
) -> Result<SendResult> {
    let url = resolve_required_secret(
        channel_name,
        "webhook_url",
        &config.webhook_url,
        &config.webhook_url_env,
    )?;
    let payload = discord_payload(
        message,
        config.allow_mentions.unwrap_or(false),
        config.username.as_deref(),
        config.avatar_url.as_deref(),
    );

    let response = if message.attachments.is_empty() {
        client.post(&url).json(&payload).send().await?
    } else {
        client
            .post(&url)
            .multipart(discord_multipart_form(&payload, &message.attachments)?)
            .send()
            .await?
    };
    let status = response.status();
    let response_text = response.text().await?;
    ensure_success("discord-webhook", status, response_text)?;

    Ok(SendResult {
        id: message.id.clone(),
        attachments: discord_sent_attachments(&message.attachments),
    })
}

pub(super) async fn send_bot(
    client: &Client,
    channel_name: &str,
    config: &DiscordBotConfig,
    message: &NotifyMessage,
) -> Result<SendResult> {
    let bot_token = resolve_required_secret(
        channel_name,
        "bot_token",
        &config.bot_token,
        &config.bot_token_env,
    )?;
    let channel_id = resolve_required_secret(
        channel_name,
        "channel_id",
        &config.channel_id,
        &config.channel_id_env,
    )?;
    let url = format!("https://discord.com/api/v10/channels/{channel_id}/messages");
    let payload = discord_payload(message, config.allow_mentions.unwrap_or(false), None, None);

    let request = client
        .post(url)
        .header(reqwest::header::AUTHORIZATION, format!("Bot {bot_token}"));
    let response = if message.attachments.is_empty() {
        request.json(&payload).send().await?
    } else {
        request
            .multipart(discord_multipart_form(&payload, &message.attachments)?)
            .send()
            .await?
    };
    let status = response.status();
    let response_text = response.text().await?;
    ensure_success("discord-bot", status, response_text)?;

    Ok(SendResult {
        id: message.id.clone(),
        attachments: discord_sent_attachments(&message.attachments),
    })
}

fn discord_payload(
    message: &NotifyMessage,
    allow_mentions: bool,
    username: Option<&str>,
    avatar_url: Option<&str>,
) -> serde_json::Value {
    let mut payload = json!({
        "content": render_message(message),
    });
    if !allow_mentions {
        payload["allowed_mentions"] = json!({ "parse": [] });
    }
    if let Some(username) = username {
        payload["username"] = json!(username);
    }
    if let Some(avatar_url) = avatar_url {
        payload["avatar_url"] = json!(avatar_url);
    }
    if !message.attachments.is_empty() {
        payload["attachments"] = json!(
            message
                .attachments
                .iter()
                .enumerate()
                .map(|(index, attachment)| json!({
                    "id": index,
                    "filename": attachment.name,
                }))
                .collect::<Vec<_>>()
        );
    }
    payload
}

fn discord_multipart_form(
    payload: &serde_json::Value,
    attachments: &[Attachment],
) -> Result<multipart::Form> {
    let mut form = multipart::Form::new().text("payload_json", serde_json::to_string(payload)?);
    for (index, attachment) in attachments.iter().enumerate() {
        form = form.part(
            format!("files[{index}]"),
            multipart_part(attachment, &attachment.name)?,
        );
    }
    Ok(form)
}

fn discord_sent_attachments(attachments: &[Attachment]) -> Vec<StoredAttachment> {
    attachments
        .iter()
        .enumerate()
        .map(|(index, attachment)| {
            StoredAttachment::sent(Some(format!("files[{index}]")), attachment)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::{MessageFormat, Priority};

    use super::*;

    #[test]
    fn discord_payload_disables_mentions_by_default() {
        let message = NotifyMessage::new(
            "Hello".to_string(),
            Some("@everyone".to_string()),
            MessageFormat::Markdown,
            Priority::Info,
            Vec::new(),
            Vec::new(),
        )
        .unwrap();

        let payload = discord_payload(&message, false, Some("Agent Notify"), None);

        assert_eq!(payload["allowed_mentions"], json!({ "parse": [] }));
        assert_eq!(payload["username"], json!("Agent Notify"));
        assert!(payload["content"].as_str().unwrap().contains("@everyone"));
    }
}
