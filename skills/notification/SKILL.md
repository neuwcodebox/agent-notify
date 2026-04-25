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
- User action or approval is required

## Core Command

```bash
notify send --channel <channel> --title "<title>" --body "<message>"
```

If the user did not specify a channel, use the configured default channel:

```bash
notify send --title "<title>" --body "<message>"
```

`--title` is required. Provide at least one of `--body`, `--body-file`, or `--file`; the CLI rejects empty notifications with no body and no attachment.

## Channel Model

A channel is a configured notification destination.

Examples:

```text
personal
team
ops
phone
local
automation
```

Use configured channel names only. Do not infer service credentials, webhook URLs, bot tokens, or destination IDs.

## Message Guidelines

Notifications should be short, clear, and actionable.

Good titles:

```text
Task completed
Task failed
Report ready
Approval required
Deployment failed
```

Good bodies:

```text
The report was generated successfully.
The build failed during the test step.
The chart image is attached.
Manual approval is required before deployment.
```

Avoid putting long raw logs in the notification body. Summarize the result instead.

## Priority

Use `--priority` when the severity matters.

Supported values:

```text
info
success
warning
error
critical
```

Examples:

```bash
notify send --channel personal --priority success --title "Task completed" --body "The report was generated successfully."
```

```bash
notify send --channel personal --priority error --title "Task failed" --body "The build failed during the test step."
```

Use priorities as follows:

```text
info      General update
success   Completed successfully
warning   Attention may be needed
error     Failed or blocked
critical  Urgent human attention required
```

## Attachments

Use `--file` to attach generated artifacts such as images, reports, or small text files.

```bash
notify send --channel personal --title "Chart ready" --body "Attached chart image." --file ./chart.png
```

Multiple files may be attached by repeating `--file`:

```bash
notify send --channel personal --title "Artifacts ready" --body "Attached generated outputs." --file ./chart.png --file ./summary.md
```

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

```bash
notify send --channel personal --title "Error summary" --body-file ./error-summary.md
```

If both `--body` and `--body-file` are provided, the CLI combines them with a blank line between the inline body and the file contents.

```bash
notify send --channel personal --title "Task failed" --body "The task failed. Summary follows." --body-file ./error-summary.md
```

## Local File Log

A configured `file-log` channel can be used for local testing or durable local records.

Example:

```bash
notify send --channel local --title "Test notification" --body "This is a local file-log notification."
```

When files are attached to a `file-log` channel, the files are copied into the channel's attachment directory and the JSONL log references the stored paths.

Use `file-log` when external delivery is unnecessary, when the user requested local logging, or when testing notification payloads.

## Webhook

A configured `webhook` channel sends notifications using the agent-notify webhook protocol.

Example:

```bash
notify send --channel automation --title "Task completed" --body "The generated report is ready."
```

With attachment:

```bash
notify send --channel automation --title "Report ready" --body "Attached generated report." --file ./report.md
```

Do not manually construct webhook payloads when the `notify` CLI is available. Let the CLI produce the standard payload.

## Dry Run

`--dry-run` is for development, configuration checks, and debugging. Do not use it as a normal preflight step before every notification.

Use it only when the user asks to test the notification setup or when you are explicitly validating a new channel configuration.

Dry run resolves the channel and renders the notification preview, but it does not send the notification, copy attachments, or prove that the destination service can receive the message. Use `notify test` when the goal is actual delivery verification.

Example:

```bash
notify send --channel personal --title "Test notification" --body "Testing notification configuration." --dry-run
```

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

## Common Examples

### Task completed

```bash
notify send --channel personal --priority success --title "Task completed" --body "The requested task completed successfully."
```

### Task failed

```bash
notify send --channel personal --priority error --title "Task failed" --body "The task failed. See the generated error summary."
```

### Approval required

```bash
notify send --channel personal --priority warning --title "Approval required" --body "Manual approval is required before continuing."
```

### Artifact ready

```bash
notify send --channel personal --priority success --title "Artifact ready" --body "The generated artifact is attached." --file ./output.png
```

### Local file log

```bash
notify send --channel local --title "Local log" --body "This notification was written to the local JSONL file log."
```

### JSON output for automation

```bash
notify send --channel personal --title "Task completed" --body "Done." --json
```

## Checking Configuration

List configured channels:

```bash
notify channels
```

Check all configured channels:

```bash
notify check
```

Check one channel:

```bash
notify check --channel personal
```

Send a test notification:

```bash
notify test --channel personal
```

Use `notify test` instead of `notify send --dry-run` when the goal is to verify that a configured channel can actually deliver a message.

## Failure Handling

If a notification command fails, do not silently ignore it.

Report:

```text
Notification failed.
Channel: <channel>
Reason: <error>
```

If the failure is caused by missing configuration, suggest checking the channel configuration or environment variable.

Example:

```text
Notification failed.
Channel: personal
Reason: missing environment variable NOTIFY_TELEGRAM_BOT_TOKEN
```
