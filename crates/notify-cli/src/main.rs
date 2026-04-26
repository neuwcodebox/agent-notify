use std::{collections::HashSet, path::PathBuf, process::ExitCode, str::FromStr};

use clap::{Args, Parser, Subcommand};
use notify_core::{
    Attachment, CheckIssue, Config, ConfigLoad, DryRunAttachment, DryRunMessage, DryRunOutput,
    DryRunResultOutput, ErrorBody, ErrorOutput, IssueLevel, MessageFormat, NotifyError,
    NotifyMessage, Priority, Result, SendOutput, SendResultOutput, config::ChannelConfig,
    provider::check_file_log_paths, send_notification,
};
use serde_json::json;
use tokio::task::JoinSet;

#[derive(Debug, Parser)]
#[command(name = "notify")]
#[command(about = "A multi-channel notification CLI for AI agents and automation scripts.")]
struct Cli {
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Send(SendArgs),
    Channels(JsonArgs),
    Check(CheckArgs),
    Test(TestArgs),
}

#[derive(Debug, Args)]
struct JsonArgs {
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct CheckArgs {
    #[arg(long)]
    channel: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct TestArgs {
    #[arg(long)]
    channel: Option<String>,
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct SendArgs {
    #[arg(long)]
    channel: Vec<String>,
    #[arg(long)]
    title: String,
    #[arg(long)]
    body: Option<String>,
    #[arg(long)]
    body_file: Option<PathBuf>,
    #[arg(long = "file")]
    files: Vec<PathBuf>,
    #[arg(long, default_value = "info")]
    priority: String,
    #[arg(long, default_value = "text")]
    format: String,
    #[arg(long = "tag")]
    tags: Vec<String>,
    #[arg(long)]
    dry_run: bool,
    #[arg(long)]
    json: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    let json_errors = cli.wants_json();

    match run(cli).await {
        Ok(code) => code,
        Err(error) => {
            if json_errors {
                let output = ErrorOutput::from(&error);
                match serde_json::to_string_pretty(&output) {
                    Ok(json) => eprintln!("{json}"),
                    Err(json_error) => eprintln!("Error: {json_error}"),
                }
            } else {
                eprintln!("Error: {error}");
            }
            ExitCode::FAILURE
        }
    }
}

impl Cli {
    fn wants_json(&self) -> bool {
        match &self.command {
            Command::Send(args) => args.json,
            Command::Channels(args) => args.json,
            Command::Check(args) => args.json,
            Command::Test(args) => args.json,
        }
    }
}

async fn run(cli: Cli) -> Result<ExitCode> {
    match cli.command {
        Command::Send(args) => {
            let loaded = ConfigLoad::load(cli.config.as_deref())?;
            run_send(&loaded.config, args).await
        }
        Command::Channels(args) => {
            let loaded = ConfigLoad::load(cli.config.as_deref())?;
            run_channels(&loaded.config, args.json)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Check(args) => {
            let loaded = ConfigLoad::load(cli.config.as_deref())?;
            run_check(&loaded.config, args)
        }
        Command::Test(args) => {
            let loaded = ConfigLoad::load(cli.config.as_deref())?;
            run_test(&loaded.config, args).await
        }
    }
}

async fn run_send(config: &Config, args: SendArgs) -> Result<ExitCode> {
    let channels = resolve_send_channels(config, &args.channel)?;
    let message = build_message(
        args.title,
        args.body,
        args.body_file,
        args.files,
        &args.priority,
        &args.format,
        args.tags,
    )?;

    if args.dry_run {
        let output = build_dry_run_output(&channels, &message);
        print_dry_run_output(&output, &message, args.json)?;
        return Ok(ExitCode::SUCCESS);
    }

    let output = send_to_channels(config, channels, &message).await?;
    let ok = output.ok;
    print_send_output(&output, &message, args.json)?;
    Ok(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

async fn run_test(config: &Config, args: TestArgs) -> Result<ExitCode> {
    run_send(
        config,
        SendArgs {
            channel: args.channel.into_iter().collect(),
            title: "agent-notify test".to_string(),
            body: Some("This is a test notification from agent-notify.".to_string()),
            body_file: None,
            files: Vec::new(),
            priority: "info".to_string(),
            format: "text".to_string(),
            tags: Vec::new(),
            dry_run: false,
            json: args.json,
        },
    )
    .await
}

async fn send_to_channels(
    config: &Config,
    channels: Vec<ResolvedChannel>,
    message: &NotifyMessage,
) -> Result<SendOutput> {
    let mut completed = Vec::new();
    let mut tasks = JoinSet::new();

    for (index, channel) in channels.into_iter().enumerate() {
        if let Err(error) = ensure_channel_ready(config, &channel.name) {
            completed.push((index, send_error_output(channel, &error)));
            continue;
        }

        let message = message.clone();
        tasks.spawn(async move {
            let result = send_notification(&channel.name, &channel.config, &message).await;
            (index, send_result_output(channel, result))
        });
    }

    while let Some(result) = tasks.join_next().await {
        completed.push(result.map_err(|error| NotifyError::Provider(error.to_string()))?);
    }

    completed.sort_by_key(|(index, _)| *index);
    let results = completed
        .into_iter()
        .map(|(_, result)| result)
        .collect::<Vec<_>>();
    let ok = results.iter().all(|result| result.ok);

    Ok(SendOutput {
        ok,
        sent: ok,
        dry_run: false,
        results,
    })
}

fn send_result_output(
    channel: ResolvedChannel,
    result: Result<notify_core::SendResult>,
) -> SendResultOutput {
    match result {
        Ok(result) => SendResultOutput {
            ok: true,
            channel: channel.name,
            channel_type: channel.channel_type,
            id: Some(result.id),
            sent: true,
            dry_run: false,
            attachments: Some(result.attachments),
            error: None,
        },
        Err(error) => send_error_output(channel, &error),
    }
}

fn send_error_output(channel: ResolvedChannel, error: &NotifyError) -> SendResultOutput {
    SendResultOutput {
        ok: false,
        channel: channel.name,
        channel_type: channel.channel_type,
        id: None,
        sent: false,
        dry_run: false,
        attachments: None,
        error: Some(ErrorBody {
            code: error.code().to_string(),
            message: error.to_string(),
        }),
    }
}

#[derive(Debug, Clone)]
struct ResolvedChannel {
    name: String,
    channel_type: String,
    config: ChannelConfig,
}

fn resolve_send_channels(config: &Config, requested: &[String]) -> Result<Vec<ResolvedChannel>> {
    let names = if requested.is_empty() {
        vec![config.resolve_channel_name(None)?.to_string()]
    } else {
        let mut seen = HashSet::new();
        for name in requested {
            if !seen.insert(name.as_str()) {
                return Err(NotifyError::InvalidInput(format!(
                    "channel \"{name}\" was specified more than once"
                )));
            }
        }
        requested
            .iter()
            .map(|name| {
                config
                    .resolve_channel_name(Some(name))
                    .map(ToOwned::to_owned)
            })
            .collect::<Result<Vec<_>>>()?
    };

    names
        .into_iter()
        .map(|name| {
            let channel = config.channel(&name)?;
            Ok(ResolvedChannel {
                name,
                channel_type: channel.type_name().to_string(),
                config: channel.clone(),
            })
        })
        .collect()
}

fn run_channels(config: &Config, json_output: bool) -> Result<()> {
    let statuses = config.channel_statuses();

    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": true,
                "channels": statuses,
            }))?
        );
        return Ok(());
    }

    for status in statuses {
        let detail = if let Some(env) = status.missing_env.first() {
            format!("missing {env}")
        } else if let Some(error) = status.errors.first() {
            error.clone()
        } else {
            status.status
        };
        println!("{:<12} {:<16} {}", status.name, status.channel_type, detail);
    }

    Ok(())
}

fn run_check(config: &Config, args: CheckArgs) -> Result<ExitCode> {
    let issues = collect_check_issues(config, args.channel.as_deref())?;
    let ok = !issues.iter().any(CheckIssue::is_error);

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "ok": ok,
                "issues": issues,
            }))?
        );
    } else if ok {
        if issues.is_empty() {
            println!("Configuration OK.");
        } else {
            println!("Configuration OK with warnings.");
            print_issues(&issues);
        }
    } else {
        println!("Configuration has errors.");
        print_issues(&issues);
    }

    Ok(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn collect_check_issues(config: &Config, channel: Option<&str>) -> Result<Vec<CheckIssue>> {
    let mut issues = if let Some(channel) = channel {
        config.channel(channel)?;
        issue_for_channel(config, channel, true)
    } else {
        let mut issues = config.validation_issues();
        issues.extend(check_file_log_paths(config));
        issues
    };
    issues.sort_by(|a, b| {
        a.code
            .cmp(&b.code)
            .then(a.channel.cmp(&b.channel))
            .then(a.message.cmp(&b.message))
    });
    Ok(issues)
}

fn build_message(
    title: String,
    body: Option<String>,
    body_file: Option<PathBuf>,
    files: Vec<PathBuf>,
    priority: &str,
    format: &str,
    tags: Vec<String>,
) -> Result<NotifyMessage> {
    let file_body = match body_file {
        Some(path) => Some(
            std::fs::read_to_string(&path).map_err(|source| NotifyError::Io { path, source })?,
        ),
        None => None,
    };
    let body = match (body, file_body) {
        (Some(body), Some(file_body)) => Some(format!("{body}\n\n{file_body}")),
        (Some(body), None) => Some(body),
        (None, Some(file_body)) => Some(file_body),
        (None, None) => None,
    };
    let attachments = files
        .iter()
        .map(Attachment::from_path)
        .collect::<Result<Vec<_>>>()?;

    NotifyMessage::new(
        title,
        body,
        MessageFormat::from_str(format)?,
        Priority::from_str(priority)?,
        tags,
        attachments,
    )
}

fn ensure_channel_ready(config: &Config, channel_name: &str) -> Result<()> {
    let issues = issue_for_channel(config, channel_name, false);
    if let Some(env) = issues
        .iter()
        .find(|issue| issue.code == "MISSING_ENV")
        .map(|issue| issue.message.clone())
    {
        return Err(NotifyError::MissingEnv {
            channel: channel_name.to_string(),
            env,
        });
    }
    if let Some(issue) = issues.iter().find(|issue| issue.level == IssueLevel::Error) {
        return Err(NotifyError::Validation(issue.message.clone()));
    }
    Ok(())
}

fn issue_for_channel(
    config: &Config,
    channel_name: &str,
    check_filesystem: bool,
) -> Vec<CheckIssue> {
    let mut issues = config
        .validation_issues()
        .into_iter()
        .filter(|issue| issue.channel.as_deref() == Some(channel_name))
        .collect::<Vec<_>>();
    if check_filesystem {
        issues.extend(
            check_file_log_paths(config)
                .into_iter()
                .filter(|issue| issue.channel.as_deref() == Some(channel_name)),
        );
    }
    issues
}

fn build_dry_run_output(channels: &[ResolvedChannel], message: &NotifyMessage) -> DryRunOutput {
    DryRunOutput {
        ok: true,
        dry_run: true,
        sent: false,
        results: channels
            .iter()
            .map(|channel| DryRunResultOutput {
                ok: true,
                channel: channel.name.clone(),
                channel_type: channel.channel_type.clone(),
                sent: false,
                dry_run: true,
                message: DryRunMessage {
                    title: message.title.clone(),
                    body: message.body.clone(),
                    format: message.format,
                    priority: message.priority,
                    tags: message.tags.clone(),
                },
                attachments: message
                    .attachments
                    .iter()
                    .map(|attachment| DryRunAttachment {
                        path: path_to_string(&attachment.path),
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn print_dry_run_output(
    output: &DryRunOutput,
    message: &NotifyMessage,
    json_output: bool,
) -> Result<()> {
    if json_output {
        println!("{}", serde_json::to_string_pretty(output)?);
        return Ok(());
    }

    println!("Dry run: no notification was sent.");
    println!();
    println!("Title: {}", message.title);
    println!("Priority: {}", message.priority);
    println!("Format: {}", message.format);
    println!(
        "Body length: {}",
        message.body.as_ref().map(|body| body.len()).unwrap_or(0)
    );
    if !message.attachments.is_empty() {
        println!("Attachments:");
        for attachment in &message.attachments {
            println!("- {}", attachment.path.display());
        }
    }
    println!("Channels:");
    for result in &output.results {
        println!("- {} ({})", result.channel, result.channel_type);
    }

    Ok(())
}

fn print_send_output(
    output: &SendOutput,
    message: &NotifyMessage,
    json_output: bool,
) -> Result<()> {
    if json_output {
        println!("{}", serde_json::to_string_pretty(output)?);
        return Ok(());
    }

    if output.ok {
        println!("Sent notification.");
    } else {
        println!("Notification completed with failures.");
    }
    println!();
    println!("Title: {}", message.title);
    println!("Channels:");
    for result in &output.results {
        if result.ok {
            println!(
                "- {} ({}) sent, attachments: {}",
                result.channel,
                result.channel_type,
                result
                    .attachments
                    .as_ref()
                    .map(|attachments| attachments.len())
                    .unwrap_or(0)
            );
        } else if let Some(error) = &result.error {
            println!(
                "- {} ({}) failed: {}",
                result.channel, result.channel_type, error.message
            );
        }
    }

    Ok(())
}

fn path_to_string(path: &std::path::Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn print_issues(issues: &[CheckIssue]) {
    for issue in issues {
        let channel = issue.channel.as_deref().unwrap_or("-");
        println!(
            "{:<7} {:<18} {:<12} {}",
            format!("{:?}", issue.level).to_lowercase(),
            issue.code,
            channel,
            issue.message
        );
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use notify_core::{
        ChannelConfig,
        config::{DiscordWebhookConfig, FileLogConfig, NtfyConfig},
    };
    use serde_json::Value;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn build_message_combines_body_and_body_file() {
        let dir = tempdir().unwrap();
        let body_file = dir.path().join("body.md");
        std::fs::write(&body_file, "from file").unwrap();

        let message = build_message(
            "Title".to_string(),
            Some("inline".to_string()),
            Some(body_file),
            Vec::new(),
            "warning",
            "markdown",
            vec!["tag".to_string()],
        )
        .unwrap();

        assert_eq!(message.body.as_deref(), Some("inline\n\nfrom file"));
        assert_eq!(message.priority, Priority::Warning);
        assert_eq!(message.format, MessageFormat::Markdown);
    }

    #[test]
    fn build_message_requires_body_or_file() {
        let error = build_message(
            "Title".to_string(),
            None,
            None,
            Vec::new(),
            "info",
            "text",
            Vec::new(),
        )
        .unwrap_err();

        assert_eq!(error.code(), "INVALID_INPUT");
    }

    #[tokio::test]
    async fn dry_run_does_not_write_file_log() {
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("notify-log");
        let mut channels = BTreeMap::new();
        channels.insert(
            "local".to_string(),
            ChannelConfig::FileLog(FileLogConfig {
                path: log_path.clone(),
            }),
        );
        let config = Config {
            default_channel: Some("local".to_string()),
            channels,
        };

        run_send(
            &config,
            SendArgs {
                channel: Vec::new(),
                title: "Dry run".to_string(),
                body: Some("No write.".to_string()),
                body_file: None,
                files: Vec::new(),
                priority: "info".to_string(),
                format: "text".to_string(),
                tags: Vec::new(),
                dry_run: true,
                json: true,
            },
        )
        .await
        .unwrap();

        assert!(!log_path.exists());
    }

    #[test]
    fn dry_run_json_uses_spec_shape() {
        let dir = tempdir().unwrap();
        let attachment_path = dir.path().join("report.txt");
        std::fs::write(&attachment_path, "hello").unwrap();
        let message = NotifyMessage::new(
            "Chart ready".to_string(),
            Some("Attached chart.".to_string()),
            MessageFormat::Text,
            Priority::Info,
            vec!["chart".to_string()],
            vec![Attachment::from_path(&attachment_path).unwrap()],
        )
        .unwrap();

        let channels = vec![ResolvedChannel {
            name: "local".to_string(),
            channel_type: "file-log".to_string(),
            config: ChannelConfig::FileLog(FileLogConfig {
                path: "notify-log".into(),
            }),
        }];
        let output = build_dry_run_output(&channels, &message);
        let json = serde_json::to_value(output).unwrap();

        assert_eq!(json["ok"], true);
        assert_eq!(json["dry_run"], true);
        assert_eq!(json["sent"], false);
        assert_eq!(json["results"][0]["ok"], true);
        assert_eq!(json["results"][0]["channel"], "local");
        assert_eq!(json["results"][0]["type"], "file-log");
        assert_eq!(json["results"][0]["sent"], false);
        assert_eq!(json["results"][0]["dry_run"], true);
        assert_eq!(json["results"][0]["message"]["title"], "Chart ready");
        assert_eq!(json["results"][0]["message"]["body"], "Attached chart.");
        assert_eq!(json["results"][0]["message"]["format"], "text");
        assert_eq!(json["results"][0]["message"]["priority"], "info");
        assert_eq!(
            json["results"][0]["message"]["tags"],
            serde_json::json!(["chart"])
        );
        assert_eq!(
            json["results"][0]["attachments"][0]["path"],
            path_to_string(&attachment_path)
        );
        assert!(json["results"][0].get("id").is_none());
    }

    #[test]
    fn sent_json_uses_spec_shape_without_dry_run_message() {
        let output = SendOutput {
            ok: true,
            sent: true,
            dry_run: false,
            results: vec![SendResultOutput {
                ok: true,
                channel: "personal".to_string(),
                channel_type: "telegram".to_string(),
                id: Some("01J00000000000000000000000".to_string()),
                sent: true,
                dry_run: false,
                attachments: Some(Vec::new()),
                error: None,
            }],
        };

        let json = serde_json::to_value(output).unwrap();

        assert_eq!(json["ok"], true);
        assert_eq!(json["sent"], true);
        assert_eq!(json["dry_run"], false);
        assert_eq!(json["results"][0]["ok"], true);
        assert_eq!(json["results"][0]["channel"], "personal");
        assert_eq!(json["results"][0]["type"], "telegram");
        assert_eq!(json["results"][0]["id"], "01J00000000000000000000000");
        assert_eq!(json["results"][0]["sent"], true);
        assert_eq!(json["results"][0]["dry_run"], false);
        assert_eq!(json["results"][0]["attachments"], serde_json::json!([]));
        assert!(json["results"][0].get("message").is_none());
    }

    #[test]
    fn default_channel_resolves_to_single_result_array() {
        let config = config_with_channels(vec![(
            "local",
            ChannelConfig::FileLog(FileLogConfig {
                path: "notify-log".into(),
            }),
        )]);
        let message = NotifyMessage::new(
            "Default".to_string(),
            Some("Uses default.".to_string()),
            MessageFormat::Text,
            Priority::Info,
            Vec::new(),
            Vec::new(),
        )
        .unwrap();

        let channels = resolve_send_channels(&config, &[]).unwrap();
        let output = build_dry_run_output(&channels, &message);
        let json = serde_json::to_value(output).unwrap();

        assert_eq!(json["ok"], true);
        assert_eq!(json["results"].as_array().unwrap().len(), 1);
        assert_eq!(json["results"][0]["channel"], "local");
        assert_eq!(json["results"][0]["type"], "file-log");
    }

    #[test]
    fn duplicate_send_channel_is_invalid_input() {
        let config = config_with_channels(vec![(
            "local",
            ChannelConfig::FileLog(FileLogConfig {
                path: "notify-log".into(),
            }),
        )]);

        let error = resolve_send_channels(&config, &["local".to_string(), "local".to_string()])
            .unwrap_err();

        assert_eq!(error.code(), "INVALID_INPUT");
        assert!(error.to_string().contains("specified more than once"));
    }

    #[tokio::test]
    async fn sends_to_multiple_file_log_channels() {
        let dir = tempdir().unwrap();
        let local_path = dir.path().join("local-log");
        let audit_path = dir.path().join("audit-log");
        let config = config_with_channels(vec![
            (
                "local",
                ChannelConfig::FileLog(FileLogConfig {
                    path: local_path.clone(),
                }),
            ),
            (
                "audit",
                ChannelConfig::FileLog(FileLogConfig {
                    path: audit_path.clone(),
                }),
            ),
        ]);
        let message = NotifyMessage::new(
            "Done".to_string(),
            Some("Task completed.".to_string()),
            MessageFormat::Text,
            Priority::Info,
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let channels =
            resolve_send_channels(&config, &["local".to_string(), "audit".to_string()]).unwrap();

        let output = send_to_channels(&config, channels, &message).await.unwrap();

        assert!(output.ok);
        assert!(output.sent);
        assert_eq!(output.results.len(), 2);
        assert_eq!(output.results[0].channel, "local");
        assert_eq!(output.results[1].channel, "audit");
        assert!(local_path.join("notifications.jsonl").exists());
        assert!(audit_path.join("notifications.jsonl").exists());
    }

    #[tokio::test]
    async fn send_collects_success_and_failure_results() {
        let dir = tempdir().unwrap();
        let local_path = dir.path().join("local-log");
        let config = config_with_channels(vec![
            (
                "local",
                ChannelConfig::FileLog(FileLogConfig {
                    path: local_path.clone(),
                }),
            ),
            (
                "phone",
                ChannelConfig::Ntfy(NtfyConfig {
                    server: None,
                    topic: None,
                    topic_env: Some("AGENT_NOTIFY_TEST_MISSING_ENV_DO_NOT_SET".to_string()),
                    token: None,
                    token_env: None,
                }),
            ),
        ]);
        let message = NotifyMessage::new(
            "Mixed".to_string(),
            Some("One channel fails.".to_string()),
            MessageFormat::Text,
            Priority::Info,
            Vec::new(),
            Vec::new(),
        )
        .unwrap();
        let channels =
            resolve_send_channels(&config, &["local".to_string(), "phone".to_string()]).unwrap();

        let output = send_to_channels(&config, channels, &message).await.unwrap();

        assert!(!output.ok);
        assert!(!output.sent);
        assert_eq!(output.results.len(), 2);
        assert!(output.results[0].ok);
        assert_eq!(output.results[0].channel, "local");
        assert!(!output.results[1].ok);
        assert_eq!(output.results[1].channel, "phone");
        assert_eq!(
            output.results[1].error.as_ref().unwrap().code,
            "MISSING_ENV"
        );
        assert!(local_path.join("notifications.jsonl").exists());
    }

    #[tokio::test]
    async fn multi_channel_dry_run_does_not_write_file_logs() {
        let dir = tempdir().unwrap();
        let local_path = dir.path().join("local-log");
        let audit_path = dir.path().join("audit-log");
        let mut channels = BTreeMap::new();
        channels.insert(
            "local".to_string(),
            ChannelConfig::FileLog(FileLogConfig {
                path: local_path.clone(),
            }),
        );
        channels.insert(
            "audit".to_string(),
            ChannelConfig::FileLog(FileLogConfig {
                path: audit_path.clone(),
            }),
        );
        let config = Config {
            default_channel: Some("local".to_string()),
            channels,
        };

        let code = run_send(
            &config,
            SendArgs {
                channel: vec!["local".to_string(), "audit".to_string()],
                title: "Dry run".to_string(),
                body: Some("No write.".to_string()),
                body_file: None,
                files: Vec::new(),
                priority: "info".to_string(),
                format: "text".to_string(),
                tags: Vec::new(),
                dry_run: true,
                json: true,
            },
        )
        .await
        .unwrap();

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(!local_path.exists());
        assert!(!audit_path.exists());
    }

    #[test]
    fn check_collects_inline_secret_missing_env_and_file_log_path_errors() {
        let dir = tempdir().unwrap();
        let file_log_path = dir.path().join("not-a-directory");
        std::fs::write(&file_log_path, "already a file").unwrap();
        let config = config_with_channels(vec![
            (
                "local",
                ChannelConfig::FileLog(FileLogConfig {
                    path: file_log_path,
                }),
            ),
            (
                "team",
                ChannelConfig::DiscordWebhook(DiscordWebhookConfig {
                    webhook_url: Some("https://example.com/webhook".to_string()),
                    webhook_url_env: None,
                    username: None,
                    avatar_url: None,
                    allow_mentions: None,
                }),
            ),
            (
                "phone",
                ChannelConfig::Ntfy(NtfyConfig {
                    server: None,
                    topic: None,
                    topic_env: Some("AGENT_NOTIFY_TEST_MISSING_ENV_DO_NOT_SET".to_string()),
                    token: None,
                    token_env: None,
                }),
            ),
        ]);

        let issues = collect_check_issues(&config, None).unwrap();

        assert_issue(&issues, "team", "INLINE_SECRET", IssueLevel::Warning);
        assert_issue(&issues, "phone", "MISSING_ENV", IssueLevel::Error);
        assert_issue(&issues, "local", "FILE_LOG_PATH_INVALID", IssueLevel::Error);
    }

    #[test]
    fn check_channel_collects_only_requested_channel_issues() {
        let config = config_with_channels(vec![
            (
                "team",
                ChannelConfig::DiscordWebhook(DiscordWebhookConfig {
                    webhook_url: Some("https://example.com/webhook".to_string()),
                    webhook_url_env: None,
                    username: None,
                    avatar_url: None,
                    allow_mentions: None,
                }),
            ),
            (
                "phone",
                ChannelConfig::Ntfy(NtfyConfig {
                    server: None,
                    topic: None,
                    topic_env: Some("AGENT_NOTIFY_TEST_MISSING_ENV_DO_NOT_SET".to_string()),
                    token: None,
                    token_env: None,
                }),
            ),
        ]);

        let issues = collect_check_issues(&config, Some("team")).unwrap();

        assert_issue(&issues, "team", "INLINE_SECRET", IssueLevel::Warning);
        assert!(
            issues
                .iter()
                .all(|issue| issue.channel.as_deref() == Some("team"))
        );
    }

    #[test]
    fn channels_json_statuses_are_stable() {
        let config = config_with_channels(vec![
            (
                "local",
                ChannelConfig::FileLog(FileLogConfig {
                    path: "notify-log".into(),
                }),
            ),
            (
                "team",
                ChannelConfig::DiscordWebhook(DiscordWebhookConfig {
                    webhook_url: Some("https://example.com/webhook".to_string()),
                    webhook_url_env: None,
                    username: None,
                    avatar_url: None,
                    allow_mentions: None,
                }),
            ),
            (
                "broken",
                ChannelConfig::DiscordWebhook(DiscordWebhookConfig {
                    webhook_url: Some("https://example.com/webhook".to_string()),
                    webhook_url_env: Some("AGENT_NOTIFY_TEST_SECRET_CONFLICT".to_string()),
                    username: None,
                    avatar_url: None,
                    allow_mentions: None,
                }),
            ),
            (
                "phone",
                ChannelConfig::Ntfy(NtfyConfig {
                    server: None,
                    topic: None,
                    topic_env: Some("AGENT_NOTIFY_TEST_MISSING_ENV_DO_NOT_SET".to_string()),
                    token: None,
                    token_env: None,
                }),
            ),
        ]);

        let json = serde_json::json!({
            "ok": true,
            "channels": config.channel_statuses(),
        });

        assert_eq!(status_for(&json, "local")["status"], "ready");
        assert_eq!(status_for(&json, "team")["status"], "ready");
        assert_eq!(status_for(&json, "broken")["status"], "error");
        assert_eq!(status_for(&json, "phone")["status"], "missing");
        assert!(status_for(&json, "team")["warnings"].is_array());
        assert!(status_for(&json, "phone")["missing_env"].is_array());
        assert_eq!(status_for(&json, "local")["type"], "file-log");
    }

    fn config_with_channels(channels: Vec<(&str, ChannelConfig)>) -> Config {
        Config {
            default_channel: Some("local".to_string()),
            channels: channels
                .into_iter()
                .map(|(name, channel)| (name.to_string(), channel))
                .collect(),
        }
    }

    fn assert_issue(issues: &[CheckIssue], channel: &str, code: &str, level: IssueLevel) {
        assert!(
            issues.iter().any(|issue| {
                issue.channel.as_deref() == Some(channel)
                    && issue.code == code
                    && issue.level == level
            }),
            "missing {level:?} issue {code} for channel {channel}: {issues:#?}"
        );
    }

    fn status_for<'a>(json: &'a Value, name: &str) -> &'a Value {
        json["channels"]
            .as_array()
            .unwrap()
            .iter()
            .find(|channel| channel["name"] == name)
            .unwrap()
    }
}
