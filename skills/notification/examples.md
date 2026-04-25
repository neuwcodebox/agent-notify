# Notification Skill Examples

Use configured channel names. Do not put webhook URLs, bot tokens, API keys, passwords, or other secrets in CLI arguments.

## Task Completed

```bash
notify send --channel personal --priority success --title "Task completed" --body "The requested task completed successfully."
```

## Task Failed

```bash
notify send --channel personal --priority error --title "Task failed" --body "The build failed during the test step."
```

## Attachment

```bash
notify send --channel personal --priority success --title "Artifact ready" --body "The generated artifact is attached." --file ./output.png
```

## Local File Log

```bash
notify send --channel local --title "Local log" --body "This notification was written to the local JSONL file log."
```

## Dry Run

```bash
notify send --channel personal --title "Preview notification" --body "This checks CLI rendering without sending." --dry-run
```

## Delivery Test

```bash
notify test --channel personal
```
