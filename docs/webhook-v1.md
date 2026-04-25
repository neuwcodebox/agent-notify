# agent-notify Webhook Protocol v1

The `webhook` channel type sends a fixed agent-notify payload to the configured URL.

Configuration:

```toml
[channels.automation]
type = "webhook"
url_env = "NOTIFY_WEBHOOK_URL"
auth_header_env = "NOTIFY_WEBHOOK_AUTH_HEADER"
timeout_seconds = 15
```

Defaults:

```text
method = POST
timeout_seconds = 15
```

When `auth_header` or `auth_header_env` is configured, its resolved value is sent as the HTTP `Authorization` header.

## JSON Request

When there are no attachments, `notify` sends `application/json`.

```json
{
  "version": "1",
  "id": "01J00000000000000000000000",
  "timestamp": "2026-04-25T12:34:56Z",
  "source": {
    "app": "agent-notify",
    "hostname": "dev-machine",
    "cwd": "/path/to/project"
  },
  "message": {
    "title": "Task completed",
    "body": "Report generated successfully.",
    "format": "text",
    "priority": "success",
    "tags": []
  },
  "attachments": []
}
```

## Multipart Request

When files are attached, `notify` sends `multipart/form-data`.

Parts:

```text
payload  application/json
file0    detected MIME type, or application/octet-stream
file1    detected MIME type, or application/octet-stream
```

The `payload` part has the same top-level shape as the JSON request. Attachment metadata references the multipart field names.

```json
{
  "version": "1",
  "id": "01J00000000000000000000000",
  "timestamp": "2026-04-25T12:34:56Z",
  "source": {
    "app": "agent-notify",
    "hostname": "dev-machine",
    "cwd": "/path/to/project"
  },
  "message": {
    "title": "Chart ready",
    "body": "Attached chart image.",
    "format": "text",
    "priority": "info",
    "tags": ["chart"]
  },
  "attachments": [
    {
      "field": "file0",
      "name": "chart.png",
      "mime_type": "image/png",
      "size_bytes": 184203,
      "sha256": "..."
    }
  ]
}
```

## Response

Success:

```json
{
  "ok": true,
  "id": "server-id-or-echo",
  "message": "accepted"
}
```

Success with warnings:

```json
{
  "ok": true,
  "id": "server-id-or-echo",
  "message": "accepted with warnings",
  "warnings": [
    "attachment preview generation failed"
  ]
}
```

Failure:

```json
{
  "ok": false,
  "error": {
    "code": "INVALID_PAYLOAD",
    "message": "message.title is required"
  }
}
```

Handling rules:

- HTTP 2xx with `ok: true` is success.
- HTTP 2xx with `ok: false` is failure.
- HTTP non-2xx is failure.
- Non-JSON responses are accepted or rejected by HTTP status.
