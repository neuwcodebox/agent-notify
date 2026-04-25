---
name: notification
description: Send notifications through configured channels using the notify CLI. Use when the user wants a notification.
---

# Notification Skill

Use the `notify` CLI to send concise notifications through configured channels.

This skill is for notifying a human or another system that something happened, such as:

- A task completed
- A task failed
- A warning needs attention
- A generated artifact is ready

## Installation

If the `notify` command is missing, install `agent-notify` from crates.io:

```bash
cargo install agent-notify
```

If `cargo` is not available or installation fails, report that `agent-notify` must be installed before sending the notification. Do not try to send notifications by manually calling provider APIs.

## Core Command

```bash
notify send --channel <channel> --title "<title>" --body "<message>"
```

If the user did not specify a channel, use the configured default channel:

```bash
notify send --title "<title>" --body "<message>"
```

`--title` is required. Provide at least one of `--body`, `--body-file`, or `--file`; the CLI rejects empty notifications with no body and no attachment.

For concrete scenario examples, see `examples.md`.

Common `notify send` options:

```text
--channel <name>       Use a configured channel. Omit to use default_channel.
--title <text>         Notification title. Required.
--body <text>          Inline notification body.
--body-file <path>     Read body from a file.
--file <path>          Attach a file. Can be repeated.
--priority <level>     info | success | warning | error | critical
--format <format>      text | markdown
--tag <tag>            Add a tag. Can be repeated.
--dry-run              Preview without sending.
--json                 Emit JSON output for automation.
--config <path>        Use a specific config file.
```

## Channel Model

A channel is a configured notification destination.

Use configured channel names only. Do not infer service credentials, webhook URLs, bot tokens, or destination IDs.

`notify` looks for configuration in this order:

1. `--config <path>`
2. `./notify.toml`
3. `~/.config/agent-notify/config.toml`

Supported channel types are `telegram`, `discord-webhook`, `discord-bot`, `ntfy`, `webhook`, and `file-log`.

## Message Guidelines

Notifications should be short, clear, and actionable.

Avoid putting long raw logs in the notification body. Summarize the result instead.

## Priority

Use `--priority` when the severity matters.

Supported values:

`info`, `success`, `warning`, `error`, `critical`

Use priorities as follows:

```text
info      General update
success   Completed successfully
warning   Attention may be needed
error     Failed or blocked
critical  Urgent human attention required
```

## Format and Tags

Use `--format text` by default. Use `--format markdown` only when the message body intentionally contains Markdown and the target channel should receive formatted content.

Use `--tag` for compact routing, filtering, or categorization labels that the configured channel can preserve. Repeat `--tag` for multiple labels.

## Attachments

Use `--file` to attach generated artifacts such as images, reports, or small text files.

Multiple files may be attached by repeating `--file`.

Do not attach private source code, credentials, databases, `.env` files, key files, or large logs unless the user explicitly requested that exact content be sent.

Attachment support by channel type:

```text
telegram          supports files and images
discord-webhook   supports files
discord-bot       supports files
webhook           supports files through the agent-notify webhook protocol
file-log          copies files into the local attachment directory
ntfy              does not support attachments
```

If the selected channel type does not support attachments, do not manually work around it. Report that the channel cannot send files.

## Body from File

Use `--body-file` when the message body was generated as a separate file.

If both `--body` and `--body-file` are provided, the CLI combines them with a blank line between the inline body and the file contents.

## Local File Log

A configured `file-log` channel can be used for local testing or durable local records.

When files are attached to a `file-log` channel, the files are copied into the channel's attachment directory and the JSONL log references the stored paths.

Use `file-log` when external delivery is unnecessary, when the user requested local logging, or when testing notification payloads.

## Webhook

A configured `webhook` channel sends notifications using the agent-notify webhook protocol.

Do not manually construct webhook payloads when the `notify` CLI is available. Let the CLI produce the standard payload.

## JSON Output

Use `--json` when another tool or script will parse the command result.

Do not use `--json` merely to make human-readable output quieter unless the user asked for machine-readable output.

## Dry Run

`--dry-run` is for development, configuration checks, and debugging. Do not use it as a normal preflight step before every notification.

Use it only when the user asks to test the notification setup or when you are explicitly validating a new channel configuration.

Dry run resolves the channel and renders the notification preview, but it does not send the notification, copy attachments, or prove that the destination service can receive the message. Use `notify test` when the goal is actual delivery verification.

## Rules

Follow these rules when using this skill:

1. Use configured channel names. Do not pass raw destination credentials in commands.
2. Do not put webhook URLs, bot tokens, API keys, passwords, or other secrets in CLI arguments.
3. Send the notification directly when the user asked for an actual notification.
4. Use `--dry-run` only for development, configuration checks, or explicit test/debug requests.
5. Prefer concise, actionable notifications.
6. Do not send private source code, credentials, databases, `.env` files, key files, or large logs unless the user explicitly requested that exact content be sent.
7. Avoid mass mentions such as `@everyone` or `@here`.
8. If the channel type does not support attachments, do not try to work around it manually. Report that the selected channel cannot send files.
9. If notification delivery fails, summarize the failure and include the channel name and error reason.

## Checking Configuration

Use `notify channels` to list configured channels and readiness.

Use `notify check` to validate configuration. Add `--channel <name>` to check only one channel. Add `--json` when automation needs structured output.

Use `notify test` instead of `notify send --dry-run` when the goal is to verify that a configured channel can actually deliver a message.

## Failure Handling

If a notification command fails, do not silently ignore it.

Report the failure with the channel name and error reason. If the failure is caused by missing configuration, suggest checking the channel configuration or environment variable.
