use reqwest::{Client, StatusCode, multipart};
use serde::Deserialize;
use serde_json::json;

use crate::{NotifyError, NotifyMessage, Result, config::TelegramConfig};

use super::{
    SendResult, StoredAttachment,
    common::{multipart_part, render_message, resolve_required_secret, trim_response_body},
};

pub(super) async fn send(
    client: &Client,
    channel_name: &str,
    config: &TelegramConfig,
    message: &NotifyMessage,
) -> Result<SendResult> {
    let bot_token = resolve_required_secret(
        channel_name,
        "bot_token",
        &config.bot_token,
        &config.bot_token_env,
    )?;
    let chat_id = resolve_required_secret(
        channel_name,
        "chat_id",
        &config.chat_id,
        &config.chat_id_env,
    )?;
    let base_url = format!("https://api.telegram.org/bot{bot_token}");
    let text = render_message(message);
    let mut payload = json!({
        "chat_id": chat_id,
        "text": text,
    });
    if let Some(parse_mode) = telegram_parse_mode(config.parse_mode.as_deref().unwrap_or("plain")) {
        payload["parse_mode"] = json!(parse_mode);
    }

    let response = client
        .post(format!("{base_url}/sendMessage"))
        .json(&payload)
        .send()
        .await?;
    handle_response(response.status(), response.text().await?)?;

    for attachment in &message.attachments {
        let (method, field_name) = if attachment.mime_type.starts_with("image/") {
            ("sendPhoto", "photo")
        } else {
            ("sendDocument", "document")
        };
        let form = multipart::Form::new()
            .text("chat_id", chat_id.clone())
            .part(field_name, multipart_part(attachment, &attachment.name)?);
        let response = client
            .post(format!("{base_url}/{method}"))
            .multipart(form)
            .send()
            .await?;
        handle_response(response.status(), response.text().await?)?;
    }

    Ok(SendResult {
        id: message.id.clone(),
        attachments: message
            .attachments
            .iter()
            .map(|attachment| StoredAttachment::sent(None, attachment))
            .collect(),
    })
}

#[derive(Debug, Deserialize)]
struct TelegramResponse {
    ok: bool,
    description: Option<String>,
}

fn handle_response(status: StatusCode, body: String) -> Result<()> {
    if !status.is_success() {
        return Err(NotifyError::Provider(format!(
            "telegram returned HTTP {status}: {}",
            trim_response_body(&body)
        )));
    }

    match serde_json::from_str::<TelegramResponse>(&body) {
        Ok(response) if response.ok => Ok(()),
        Ok(response) => {
            Err(NotifyError::Provider(response.description.unwrap_or_else(
                || "telegram returned ok=false".to_string(),
            )))
        }
        Err(_) => Err(NotifyError::Provider(
            "telegram returned a non-JSON response".to_string(),
        )),
    }
}

fn telegram_parse_mode(parse_mode: &str) -> Option<&'static str> {
    match parse_mode {
        "plain" => None,
        "html" => Some("HTML"),
        "markdown-v2" => Some("MarkdownV2"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telegram_parse_mode_maps_supported_values() {
        assert_eq!(telegram_parse_mode("plain"), None);
        assert_eq!(telegram_parse_mode("html"), Some("HTML"));
        assert_eq!(telegram_parse_mode("markdown-v2"), Some("MarkdownV2"));
    }
}
