use std::{path::PathBuf, process::ExitCode, str::FromStr};

use clap::{Args, Parser, Subcommand};
use notify_core::{
    Attachment, CheckIssue, Config, ConfigLoad, ErrorOutput, IssueLevel, MessageFormat,
    NotifyError, NotifyMessage, Priority, Result, SendOutput,
    provider::{StoredAttachment, check_file_log_paths},
    send_notification,
};
use serde_json::json;

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
    channel: Option<String>,
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

fn main() -> ExitCode {
    let cli = Cli::parse();
    let json_errors = cli.wants_json();

    match run(cli) {
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

fn run(cli: Cli) -> Result<ExitCode> {
    match cli.command {
        Command::Send(args) => {
            let loaded = ConfigLoad::load(cli.config.as_deref())?;
            run_send(&loaded.config, args)?;
            Ok(ExitCode::SUCCESS)
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
            run_test(&loaded.config, args)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn run_send(config: &Config, args: SendArgs) -> Result<()> {
    let channel_name = config.resolve_channel_name(args.channel.as_deref())?;
    let channel = config.channel(channel_name)?;
    ensure_channel_ready(config, channel_name)?;

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
        let output = SendOutput {
            ok: true,
            channel: channel_name.to_string(),
            channel_type: channel.type_name().to_string(),
            id: message.id.clone(),
            sent: false,
            dry_run: true,
            attachments: message
                .attachments
                .iter()
                .map(|attachment| StoredAttachment::dry_run(&attachment.path))
                .collect(),
        };
        print_send_output(&output, &message, args.json);
        return Ok(());
    }

    let result = send_notification(channel_name, channel, &message)?;
    let output = SendOutput {
        ok: true,
        channel: channel_name.to_string(),
        channel_type: channel.type_name().to_string(),
        id: result.id,
        sent: true,
        dry_run: false,
        attachments: result.attachments,
    };
    print_send_output(&output, &message, args.json);
    Ok(())
}

fn run_test(config: &Config, args: TestArgs) -> Result<()> {
    run_send(
        config,
        SendArgs {
            channel: args.channel,
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
    let mut issues = if let Some(channel) = args.channel.as_deref() {
        config.channel(channel)?;
        issue_for_channel(config, channel, true)
    } else {
        let mut issues = config.validation_issues();
        issues.extend(check_file_log_paths(config));
        issues
    };
    issues.sort_by(|a, b| a.code.cmp(&b.code).then(a.message.cmp(&b.message)));
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

fn print_send_output(output: &SendOutput, message: &NotifyMessage, json_output: bool) {
    if json_output {
        println!("{}", serde_json::to_string_pretty(output).unwrap());
        return;
    }

    if output.dry_run {
        println!("Dry run: no notification was sent.");
        println!();
        println!("Channel: {}", output.channel);
        println!("Type: {}", output.channel_type);
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
    } else {
        println!("Sent notification.");
        println!();
        println!("Channel: {}", output.channel);
        println!("Type: {}", output.channel_type);
        println!("Title: {}", message.title);
        println!("Attachments: {}", output.attachments.len());
    }
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

    use notify_core::{ChannelConfig, config::FileLogConfig};
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

    #[test]
    fn dry_run_does_not_write_file_log() {
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
                channel: None,
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
        .unwrap();

        assert!(!log_path.exists());
    }
}
