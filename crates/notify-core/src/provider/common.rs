use std::{fs, path::Path};

use reqwest::{StatusCode, multipart};

use crate::{Attachment, NotifyError, NotifyMessage, Result};

pub(super) fn multipart_part(attachment: &Attachment, file_name: &str) -> Result<multipart::Part> {
    let bytes = fs::read(&attachment.path).map_err(|source| NotifyError::Io {
        path: attachment.path.clone(),
        source,
    })?;
    Ok(multipart::Part::bytes(bytes)
        .file_name(file_name.to_string())
        .mime_str(&attachment.mime_type)?)
}

pub(super) fn render_message(message: &NotifyMessage) -> String {
    match message.body.as_deref() {
        Some(body) if !body.is_empty() => {
            format!("[{}] {}\n\n{body}", message.priority, message.title)
        }
        _ => format!("[{}] {}", message.priority, message.title),
    }
}

pub(super) fn resolve_required_secret(
    channel_name: &str,
    field: &str,
    inline: &Option<String>,
    env_name: &Option<String>,
) -> Result<String> {
    resolve_secret(channel_name, field, inline, env_name, true)?.ok_or_else(|| {
        NotifyError::Validation(format!(
            "channel \"{channel_name}\" is missing {field} or {field}_env"
        ))
    })
}

pub(super) fn resolve_optional_secret(
    channel_name: &str,
    field: &str,
    inline: &Option<String>,
    env_name: &Option<String>,
) -> Result<Option<String>> {
    resolve_secret(channel_name, field, inline, env_name, false)
}

fn resolve_secret(
    channel_name: &str,
    field: &str,
    inline: &Option<String>,
    env_name: &Option<String>,
    required: bool,
) -> Result<Option<String>> {
    match (inline.as_deref(), env_name.as_deref()) {
        (Some(_), Some(_)) => Err(NotifyError::Validation(format!(
            "channel \"{channel_name}\" {field} and {field}_env cannot be set at the same time"
        ))),
        (Some(value), None) => Ok(Some(value.to_string())),
        (None, Some(env_name)) => {
            std::env::var(env_name)
                .map(Some)
                .map_err(|_| NotifyError::MissingEnv {
                    channel: channel_name.to_string(),
                    env: env_name.to_string(),
                })
        }
        (None, None) if required => Err(NotifyError::Validation(format!(
            "channel \"{channel_name}\" is missing {field} or {field}_env"
        ))),
        (None, None) => Ok(None),
    }
}

pub(super) fn ensure_success(provider: &str, status: StatusCode, body: String) -> Result<()> {
    if status.is_success() {
        Ok(())
    } else {
        Err(NotifyError::Provider(format!(
            "{provider} returned HTTP {status}: {}",
            trim_response_body(&body)
        )))
    }
}

pub(super) fn trim_response_body(body: &str) -> String {
    const MAX_LEN: usize = 240;
    let body = body.trim();
    if body.len() > MAX_LEN {
        format!("{}...", &body[..MAX_LEN])
    } else {
        body.to_string()
    }
}

pub(super) fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_resolution_does_not_leak_value_in_conflict_error() {
        let error = resolve_required_secret(
            "team",
            "webhook_url",
            &Some("super-secret".to_string()),
            &Some("NOTIFY_SECRET".to_string()),
        )
        .unwrap_err();

        assert!(!error.to_string().contains("super-secret"));
        assert!(error.to_string().contains("webhook_url_env"));
    }
}
