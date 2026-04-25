# Notification Skill Examples

Use configured channel names. Do not put webhook URLs, bot tokens, API keys, passwords, or other secrets in CLI arguments.

## Default Channel

```bash
notify send --title "Task completed" --body "The requested task completed successfully."
```

## Task Completed

```bash
notify send --channel personal --priority success --title "Task completed" --body "The requested task completed successfully."
```

## Task Failed

```bash
notify send --channel personal --priority error --title "Task failed" --body "The build failed during the test step."
```

```bash
notify send --channel personal --priority error --title "Task failed" --body "The task failed. See the generated error summary."
```

## Warning

```bash
notify send --channel personal --priority warning --title "Warning" --body "Attention may be needed."
```

## Critical

```bash
notify send --channel personal --priority critical --title "Urgent attention required" --body "The task is blocked and needs human review."
```

## Attachment

```bash
notify send --channel personal --priority success --title "Artifact ready" --body "The generated artifact is attached." --file ./output.png
```

```bash
notify send --channel personal --title "Chart ready" --body "Attached chart image." --file ./chart.png
```

## Multiple Attachments

```bash
notify send --channel personal --title "Artifacts ready" --body "Attached generated outputs." --file ./chart.png --file ./summary.md
```

## Body From File

```bash
notify send --channel personal --title "Error summary" --body-file ./error-summary.md
```

```bash
notify send --channel personal --title "Task failed" --body "The task failed. Summary follows." --body-file ./error-summary.md
```

## Markdown Format

```bash
notify send --channel personal --title "Report ready" --body-file ./summary.md --format markdown
```

```bash
notify send --channel personal --title "Release notes" --body "See the generated notes below." --body-file ./release-notes.md --format markdown
```

## Tags

```bash
notify send --channel personal --title "Build complete" --body "The build completed successfully." --tag build --tag complete
```

```bash
notify send --channel personal --priority error --title "Deploy failed" --body "The deploy step failed." --tag deploy --tag production
```

## Local File Log

```bash
notify send --channel local --title "Local log" --body "This notification was written to the local JSONL file log."
```

```bash
notify send --channel local --title "Test notification" --body "This is a local file-log notification."
```

## Webhook

```bash
notify send --channel automation --title "Task completed" --body "The generated report is ready."
```

```bash
notify send --channel automation --title "Report ready" --body "Attached generated report." --file ./report.md
```

## JSON Output For Automation

```bash
notify send --channel personal --title "Task completed" --body "Done." --json
```

```bash
notify channels --json
```

```bash
notify check --channel personal --json
```

```bash
notify test --channel personal --json
```

## Specific Config File

```bash
notify --config ./notify.toml send --channel personal --title "Task completed" --body "Done."
```

```bash
notify --config ./notify.toml check --channel personal
```

## Dry Run

```bash
notify send --channel personal --title "Preview notification" --body "This checks CLI rendering without sending." --dry-run
```

```bash
notify send --channel personal --title "Test notification" --body "Testing notification configuration." --dry-run
```

## List Channels

```bash
notify channels
```

## Check All Channels

```bash
notify check
```

## Check One Channel

```bash
notify check --channel personal
```

## Delivery Test

```bash
notify test --channel personal
```

## Failure Report

```text
Notification failed.
Channel: personal
Reason: missing environment variable NOTIFY_TELEGRAM_BOT_TOKEN
```
