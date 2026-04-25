use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{NotifyError, Result};

#[derive(Debug, Clone)]
pub struct ConfigLoad {
    pub config: Config,
    pub path: PathBuf,
}

impl ConfigLoad {
    pub fn load(explicit_path: Option<&Path>) -> Result<Self> {
        let path = discover_config_path(explicit_path)?;
        let contents = fs::read_to_string(&path).map_err(|source| NotifyError::ConfigRead {
            path: path.clone(),
            source,
        })?;
        let config = toml::from_str(&contents).map_err(|source| NotifyError::ConfigParse {
            path: path.clone(),
            source,
        })?;

        Ok(Self { config, path })
    }
}

pub fn discover_config_path(explicit_path: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = explicit_path {
        return Ok(path.to_path_buf());
    }

    let local = PathBuf::from("notify.toml");
    if local.exists() {
        return Ok(local);
    }

    if let Some(home) = dirs::home_dir() {
        let path = home
            .join(".config")
            .join("agent-notify")
            .join("config.toml");
        if path.exists() {
            return Ok(path);
        }
    }

    Err(NotifyError::ConfigNotFound)
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub default_channel: Option<String>,
    #[serde(default)]
    pub channels: BTreeMap<String, ChannelConfig>,
}

impl Config {
    pub fn resolve_channel_name<'a>(&'a self, requested: Option<&'a str>) -> Result<&'a str> {
        let name = match requested {
            Some(name) => name,
            None => self
                .default_channel
                .as_deref()
                .ok_or(NotifyError::DefaultChannelMissing)?,
        };

        if self.channels.contains_key(name) {
            Ok(name)
        } else {
            Err(NotifyError::ChannelNotFound(name.to_string()))
        }
    }

    pub fn channel(&self, name: &str) -> Result<&ChannelConfig> {
        self.channels
            .get(name)
            .ok_or_else(|| NotifyError::ChannelNotFound(name.to_string()))
    }

    pub fn validation_issues(&self) -> Vec<CheckIssue> {
        self.validation_issues_with(&ProcessEnv)
    }

    pub fn validation_issues_with<E: EnvSource>(&self, env: &E) -> Vec<CheckIssue> {
        let mut issues = Vec::new();

        match self.default_channel.as_deref() {
            Some(name) if !self.channels.contains_key(name) => {
                issues.push(CheckIssue::error(
                    None,
                    "DEFAULT_CHANNEL_NOT_FOUND",
                    format!("default_channel \"{name}\" does not exist"),
                ));
            }
            None => {
                issues.push(CheckIssue::error(
                    None,
                    "DEFAULT_CHANNEL_MISSING",
                    "default_channel is not configured",
                ));
            }
            Some(_) => {}
        }

        for (name, channel) in &self.channels {
            issues.extend(channel.validation_issues(name, env));
        }

        issues
    }

    pub fn channel_statuses(&self) -> Vec<ChannelStatus> {
        self.channel_statuses_with(&ProcessEnv)
    }

    pub fn channel_statuses_with<E: EnvSource>(&self, env: &E) -> Vec<ChannelStatus> {
        self.channels
            .iter()
            .map(|(name, channel)| {
                let issues = channel.validation_issues(name, env);
                let missing_env = issues
                    .iter()
                    .filter(|issue| issue.code == "MISSING_ENV")
                    .map(|issue| issue.message.clone())
                    .collect::<Vec<_>>();
                let warnings = issues
                    .iter()
                    .filter(|issue| issue.level == IssueLevel::Warning)
                    .map(|issue| issue.message.clone())
                    .collect::<Vec<_>>();
                let errors = issues
                    .iter()
                    .filter(|issue| issue.level == IssueLevel::Error)
                    .map(|issue| issue.message.clone())
                    .collect::<Vec<_>>();
                let status = if errors.is_empty() {
                    "ready"
                } else if !missing_env.is_empty() {
                    "missing"
                } else {
                    "error"
                };

                ChannelStatus {
                    name: name.clone(),
                    channel_type: channel.type_name().to_string(),
                    status: status.to_string(),
                    missing_env,
                    warnings,
                    errors,
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ChannelConfig {
    Telegram(TelegramConfig),
    DiscordWebhook(DiscordWebhookConfig),
    DiscordBot(DiscordBotConfig),
    Ntfy(NtfyConfig),
    Webhook(WebhookConfig),
    FileLog(FileLogConfig),
}

impl ChannelConfig {
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Telegram(_) => "telegram",
            Self::DiscordWebhook(_) => "discord-webhook",
            Self::DiscordBot(_) => "discord-bot",
            Self::Ntfy(_) => "ntfy",
            Self::Webhook(_) => "webhook",
            Self::FileLog(_) => "file-log",
        }
    }

    fn validation_issues<E: EnvSource>(&self, channel: &str, env: &E) -> Vec<CheckIssue> {
        match self {
            Self::Telegram(config) => config.validation_issues(channel, env),
            Self::DiscordWebhook(config) => config.validation_issues(channel, env),
            Self::DiscordBot(config) => config.validation_issues(channel, env),
            Self::Ntfy(config) => config.validation_issues(channel, env),
            Self::Webhook(config) => config.validation_issues(channel, env),
            Self::FileLog(config) => config.validation_issues(channel),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct TelegramConfig {
    pub bot_token: Option<String>,
    pub bot_token_env: Option<String>,
    pub chat_id: Option<String>,
    pub chat_id_env: Option<String>,
    pub parse_mode: Option<String>,
}

impl TelegramConfig {
    fn validation_issues<E: EnvSource>(&self, channel: &str, env: &E) -> Vec<CheckIssue> {
        let mut issues = Vec::new();
        validate_secret_pair(
            channel,
            "bot_token",
            self.bot_token.as_deref(),
            self.bot_token_env.as_deref(),
            true,
            env,
            &mut issues,
        );
        validate_secret_pair(
            channel,
            "chat_id",
            self.chat_id.as_deref(),
            self.chat_id_env.as_deref(),
            true,
            env,
            &mut issues,
        );

        if let Some(parse_mode) = self.parse_mode.as_deref()
            && !matches!(parse_mode, "plain" | "html" | "markdown-v2")
        {
            issues.push(CheckIssue::error(
                Some(channel),
                "INVALID_FIELD",
                format!("channel \"{channel}\" has invalid parse_mode \"{parse_mode}\""),
            ));
        }

        issues
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiscordWebhookConfig {
    pub webhook_url: Option<String>,
    pub webhook_url_env: Option<String>,
    pub username: Option<String>,
    pub avatar_url: Option<String>,
    pub allow_mentions: Option<bool>,
}

impl DiscordWebhookConfig {
    fn validation_issues<E: EnvSource>(&self, channel: &str, env: &E) -> Vec<CheckIssue> {
        let mut issues = Vec::new();
        validate_secret_pair(
            channel,
            "webhook_url",
            self.webhook_url.as_deref(),
            self.webhook_url_env.as_deref(),
            true,
            env,
            &mut issues,
        );
        issues
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiscordBotConfig {
    pub bot_token: Option<String>,
    pub bot_token_env: Option<String>,
    pub channel_id: Option<String>,
    pub channel_id_env: Option<String>,
    pub allow_mentions: Option<bool>,
}

impl DiscordBotConfig {
    fn validation_issues<E: EnvSource>(&self, channel: &str, env: &E) -> Vec<CheckIssue> {
        let mut issues = Vec::new();
        validate_secret_pair(
            channel,
            "bot_token",
            self.bot_token.as_deref(),
            self.bot_token_env.as_deref(),
            true,
            env,
            &mut issues,
        );
        validate_secret_pair(
            channel,
            "channel_id",
            self.channel_id.as_deref(),
            self.channel_id_env.as_deref(),
            true,
            env,
            &mut issues,
        );
        issues
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct NtfyConfig {
    pub server: Option<String>,
    pub topic: Option<String>,
    pub topic_env: Option<String>,
    pub token: Option<String>,
    pub token_env: Option<String>,
}

impl NtfyConfig {
    fn validation_issues<E: EnvSource>(&self, channel: &str, env: &E) -> Vec<CheckIssue> {
        let mut issues = Vec::new();
        validate_secret_pair(
            channel,
            "topic",
            self.topic.as_deref(),
            self.topic_env.as_deref(),
            true,
            env,
            &mut issues,
        );
        validate_secret_pair(
            channel,
            "token",
            self.token.as_deref(),
            self.token_env.as_deref(),
            false,
            env,
            &mut issues,
        );
        issues
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct WebhookConfig {
    pub url: Option<String>,
    pub url_env: Option<String>,
    pub auth_header: Option<String>,
    pub auth_header_env: Option<String>,
    pub timeout_seconds: Option<u64>,
}

impl WebhookConfig {
    fn validation_issues<E: EnvSource>(&self, channel: &str, env: &E) -> Vec<CheckIssue> {
        let mut issues = Vec::new();
        validate_secret_pair(
            channel,
            "url",
            self.url.as_deref(),
            self.url_env.as_deref(),
            true,
            env,
            &mut issues,
        );
        validate_secret_pair(
            channel,
            "auth_header",
            self.auth_header.as_deref(),
            self.auth_header_env.as_deref(),
            false,
            env,
            &mut issues,
        );
        if matches!(self.timeout_seconds, Some(0)) {
            issues.push(CheckIssue::error(
                Some(channel),
                "INVALID_FIELD",
                format!("channel \"{channel}\" timeout_seconds must be greater than 0"),
            ));
        }
        issues
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileLogConfig {
    pub path: PathBuf,
}

impl FileLogConfig {
    fn validation_issues(&self, channel: &str) -> Vec<CheckIssue> {
        if self.path.as_os_str().is_empty() {
            vec![CheckIssue::error(
                Some(channel),
                "MISSING_FIELD",
                format!("channel \"{channel}\" is missing path"),
            )]
        } else {
            Vec::new()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueLevel {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckIssue {
    pub level: IssueLevel,
    pub channel: Option<String>,
    pub code: String,
    pub message: String,
}

impl CheckIssue {
    pub fn error(channel: Option<&str>, code: &str, message: impl Into<String>) -> Self {
        Self {
            level: IssueLevel::Error,
            channel: channel.map(ToOwned::to_owned),
            code: code.to_string(),
            message: message.into(),
        }
    }

    pub fn warning(channel: Option<&str>, code: &str, message: impl Into<String>) -> Self {
        Self {
            level: IssueLevel::Warning,
            channel: channel.map(ToOwned::to_owned),
            code: code.to_string(),
            message: message.into(),
        }
    }

    pub fn is_error(&self) -> bool {
        self.level == IssueLevel::Error
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ChannelStatus {
    pub name: String,
    #[serde(rename = "type")]
    pub channel_type: String,
    pub status: String,
    pub missing_env: Vec<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

pub trait EnvSource {
    fn exists(&self, name: &str) -> bool;
}

#[derive(Debug, Clone, Copy)]
pub struct ProcessEnv;

impl EnvSource for ProcessEnv {
    fn exists(&self, name: &str) -> bool {
        std::env::var_os(name).is_some()
    }
}

fn validate_secret_pair<E: EnvSource>(
    channel: &str,
    field: &str,
    inline: Option<&str>,
    env_name: Option<&str>,
    required: bool,
    env: &E,
    issues: &mut Vec<CheckIssue>,
) {
    match (inline, env_name) {
        (Some(_), Some(_)) => issues.push(CheckIssue::error(
            Some(channel),
            "SECRET_CONFLICT",
            format!("channel \"{channel}\" {field} and {field}_env cannot be set at the same time"),
        )),
        (Some(_), None) => issues.push(CheckIssue::warning(
            Some(channel),
            "INLINE_SECRET",
            format!("channel \"{channel}\" uses inline {field}"),
        )),
        (None, Some(env_name)) if !env.exists(env_name) => issues.push(CheckIssue::error(
            Some(channel),
            "MISSING_ENV",
            env_name.to_string(),
        )),
        (None, None) if required => issues.push(CheckIssue::error(
            Some(channel),
            "MISSING_FIELD",
            format!("channel \"{channel}\" is missing {field} or {field}_env"),
        )),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, fs};

    use tempfile::tempdir;

    use super::*;

    struct MapEnv(BTreeSet<String>);

    impl EnvSource for MapEnv {
        fn exists(&self, name: &str) -> bool {
            self.0.contains(name)
        }
    }

    #[test]
    fn loads_config_from_explicit_path() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("notify.toml");
        fs::write(
            &path,
            r#"
default_channel = "local"

[channels.local]
type = "file-log"
path = "./notify-log"
"#,
        )
        .unwrap();

        let loaded = ConfigLoad::load(Some(&path)).unwrap();

        assert_eq!(loaded.path, path);
        assert_eq!(loaded.config.default_channel.as_deref(), Some("local"));
        assert!(matches!(
            loaded.config.channels.get("local"),
            Some(ChannelConfig::FileLog(_))
        ));
    }

    #[test]
    fn detects_default_channel_missing() {
        let config: Config = toml::from_str(
            r#"
[channels.local]
type = "file-log"
path = "./notify-log"
"#,
        )
        .unwrap();

        let issues = config.validation_issues_with(&MapEnv(BTreeSet::new()));

        assert!(
            issues
                .iter()
                .any(|issue| issue.code == "DEFAULT_CHANNEL_MISSING")
        );
    }

    #[test]
    fn detects_default_channel_not_found() {
        let config: Config = toml::from_str(
            r#"
default_channel = "missing"

[channels.local]
type = "file-log"
path = "./notify-log"
"#,
        )
        .unwrap();

        let issues = config.validation_issues_with(&MapEnv(BTreeSet::new()));

        assert!(issues.iter().any(|issue| {
            issue.code == "DEFAULT_CHANNEL_NOT_FOUND" && issue.level == IssueLevel::Error
        }));
    }

    #[test]
    fn detects_secret_conflict_and_missing_env() {
        let config: Config = toml::from_str(
            r#"
default_channel = "team"

[channels.team]
type = "discord-webhook"
webhook_url = "https://example.com"
webhook_url_env = "NOTIFY_DISCORD_WEBHOOK_URL"

[channels.phone]
type = "ntfy"
topic_env = "NOTIFY_NTFY_TOPIC"
"#,
        )
        .unwrap();

        let issues = config.validation_issues_with(&MapEnv(BTreeSet::new()));

        assert!(issues.iter().any(|issue| issue.code == "SECRET_CONFLICT"));
        assert!(issues.iter().any(|issue| issue.code == "MISSING_ENV"));
    }

    #[test]
    fn detects_invalid_telegram_parse_mode() {
        let config: Config = toml::from_str(
            r#"
default_channel = "personal"

[channels.personal]
type = "telegram"
bot_token = "token"
chat_id = "chat"
parse_mode = "markdown"
"#,
        )
        .unwrap();

        let issues = config.validation_issues_with(&MapEnv(BTreeSet::new()));

        assert!(issues.iter().any(|issue| {
            issue.code == "INVALID_FIELD"
                && issue.level == IssueLevel::Error
                && issue.channel.as_deref() == Some("personal")
        }));
    }

    #[test]
    fn rejects_unsupported_type_during_deserialize() {
        let result = toml::from_str::<Config>(
            r#"
default_channel = "mail"

[channels.mail]
type = "email"
"#,
        );

        assert!(result.is_err());
    }
}
