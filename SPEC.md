# agent-notify 기획안

## 1. 목적

`agent-notify`는 AI Agent와 자동화 스크립트가 다양한 알림 경로로 메시지와 산출물을 보낼 수 있게 해주는 Rust 기반 CLI 도구다.

핵심 목표는 다음이다.

```text
- Agent가 단순한 CLI 명령으로 알림을 보낼 수 있게 한다.
- Discord, Telegram, ntfy, webhook, local file log 등을 통합 인터페이스로 지원한다.
- 알림 목적지는 channel 이름으로 추상화한다.
- 실제 전송 방식은 type으로 구분한다.
- 비밀값은 환경변수 참조를 권장하되, 간단한 사용을 위해 inline 값도 허용한다.
- Agent Skills에서 사용법과 안전한 관례를 명확히 제공한다.
```

이 도구는 “사람이 직접 쓰는 알림 CLI”이기도 하지만, 주된 설계 기준은 **AI Agent가 안정적으로 사용할 수 있는 알림 인터페이스**다.

---

## 2. 프로젝트 이름

```text
Project name: agent-notify
Binary name: notify
Language: Rust
Config format: TOML
Skill name: notification
```

한 줄 설명:

```text
A multi-channel notification CLI for AI agents and automation scripts.
```

---

## 3. 핵심 개념

### 3.1 channel

`channel`은 사용자가 선택하는 논리적 알림 목적지 이름이다.

예:

```text
personal
team
ops
phone
local
webhook-dev
```

Agent는 실제 구현 방식을 몰라도 된다.

```bash
notify send --channel personal --title "Task completed" --body "Report generated."
```

### 3.2 type

`type`은 해당 channel의 실제 전송 방식이다.

예:

```text
telegram
discord-webhook
discord-bot
ntfy
webhook
file-log
```

설정 예:

```toml
[channels.personal]
type = "telegram"

[channels.team]
type = "discord-webhook"

[channels.local]
type = "file-log"
```

### 3.3 provider

`provider`는 코드 내부에서 `type`별 구현체를 가리키는 개념이다.

예:

```text
TelegramProvider
DiscordWebhookProvider
DiscordBotProvider
NtfyProvider
WebhookProvider
FileLogProvider
```

사용자 문서에서는 가능하면 `provider`라는 말을 노출하지 않는다. 사용자에게는 `channel`과 `type`만 설명한다.

---

## 4. v1 지원 channel type

v1에서 지원할 type은 다음으로 확정한다.

```text
telegram
discord-webhook
discord-bot
ntfy
webhook
file-log
```

제외 또는 보류:

```text
email       -> 후순위
gotify      -> v1 제외
slack       -> 후순위
teams       -> 후순위
mattermost  -> 후순위
desktop     -> 제외, file-log로 대체
stdout      -> 제외
```

---

## 5. CLI 명령

## 5.1 notify send

알림을 전송한다.

```bash
notify send \
  --channel personal \
  --title "Task completed" \
  --body "The report was generated successfully."
```

옵션:

```text
--channel <name>       사용할 channel 이름. 생략 시 default_channel 사용
--title <text>         알림 제목
--body <text>          알림 본문
--body-file <path>     본문을 파일에서 읽음
--file <path>          첨부 파일. 여러 번 지정 가능
--priority <level>     info | success | warning | error | critical
--format <format>      text | markdown
--tag <tag>            태그. 여러 번 지정 가능
--dry-run              실제 전송하지 않고 해석 결과만 출력
--json                 결과를 JSON으로 출력
--config <path>        설정 파일 경로 지정
```

기본값:

```text
priority = "info"
format = "text"
```

본문 규칙:

```text
- --body만 있으면 해당 문자열을 본문으로 사용한다.
- --body-file만 있으면 파일 내용을 본문으로 사용한다.
- --body와 --body-file이 둘 다 있으면 body 뒤에 빈 줄을 넣고 body-file 내용을 이어붙인다.
- title은 필수다.
- body 또는 body-file은 선택이다. 단, body가 없고 file도 없으면 경고 또는 에러 처리한다.
```

권장: body도 file도 없는 알림은 실수일 가능성이 높으므로 에러 처리한다.

```text
title required
body or file required
```

---

## 5.2 notify channels

설정된 channel 목록을 보여준다.

```bash
notify channels
```

예상 출력:

```text
personal     telegram          ready
team         discord-webhook   ready
bot-team     discord-bot       missing NOTIFY_DISCORD_BOT_TOKEN
phone        ntfy              ready
local        file-log          ready
automation   webhook           ready
```

`--json` 지원:

```bash
notify channels --json
```

---

## 5.3 notify check

설정이 유효한지 검사한다.

```bash
notify check
notify check --channel personal
```

검사 항목:

```text
- 설정 파일 파싱 가능 여부
- default_channel 존재 여부
- channel type 지원 여부
- 필수 필드 존재 여부
- *_env 참조 환경변수 존재 여부
- inline secret 사용 여부 warning
- file-log 저장 경로 생성 가능 여부
```

중요: 이 명령은 과도한 보안 정책을 강제하지 않는다. 사용자가 설정한 값을 “검사하고 알려주는” 수준에 머문다.

---

## 5.4 notify test

테스트 알림을 보낸다.

```bash
notify test --channel personal
```

전송 내용:

```text
Title: agent-notify test
Body: This is a test notification from agent-notify.
Priority: info
```

---

## 5.5 notify config init

선택 기능이다. v1에 포함해도 되고, 구현 부담이 있으면 v1.1로 미룬다.

```bash
notify config init
```

기본 설정 파일 예시를 생성한다.

---

# 6. 설정 파일

기본 설정 파일 탐색 순서:

```text
1. --config로 지정한 경로
2. ./notify.toml
3. ~/.config/agent-notify/config.toml
```

설정 파일 예:

```toml
default_channel = "personal"

[channels.personal]
type = "telegram"
bot_token_env = "NOTIFY_TELEGRAM_BOT_TOKEN"
chat_id_env = "NOTIFY_TELEGRAM_CHAT_ID"
parse_mode = "plain"

[channels.team]
type = "discord-webhook"
webhook_url_env = "NOTIFY_DISCORD_WEBHOOK_URL"
username = "Agent Notify"
allow_mentions = false

[channels.bot_team]
type = "discord-bot"
bot_token_env = "NOTIFY_DISCORD_BOT_TOKEN"
channel_id_env = "NOTIFY_DISCORD_CHANNEL_ID"
allow_mentions = false

[channels.phone]
type = "ntfy"
server = "https://ntfy.sh"
topic_env = "NOTIFY_NTFY_TOPIC"
token_env = "NOTIFY_NTFY_TOKEN"

[channels.automation]
type = "webhook"
url_env = "NOTIFY_WEBHOOK_URL"
auth_header_env = "NOTIFY_WEBHOOK_AUTH_HEADER"

[channels.local]
type = "file-log"
path = "./notify-log"
```

---

# 7. secret 설정 규칙

민감값은 두 가지 형태를 지원한다.

```text
field = "inline value"
field_env = "ENV_VAR_NAME"
```

예:

```toml
webhook_url = "https://discord.com/api/webhooks/..."
webhook_url_env = "NOTIFY_DISCORD_WEBHOOK_URL"
```

둘 다 설정하면 에러 처리한다.

```text
webhook_url and webhook_url_env cannot be set at the same time.
```

이유:

```text
- 어느 값이 사용되는지 모호해지지 않게 하기 위함
- _env 우선 같은 암묵적 규칙을 피하기 위함
```

CLI 인자로 secret을 받는 옵션은 제공하지 않는다.

금지 예:

```bash
notify send --telegram-bot-token "..."
notify send --discord-webhook-url "..."
```

허용 예:

```toml
[channels.quick]
type = "discord-webhook"
webhook_url = "https://discord.com/api/webhooks/..."
```

권장 예:

```toml
[channels.team]
type = "discord-webhook"
webhook_url_env = "NOTIFY_DISCORD_WEBHOOK_URL"
```

문서 방침:

```text
- README에서는 inline 설정과 env 설정을 모두 설명한다.
- Agent Skill에서는 env 방식만 권장한다.
- notify check는 inline secret 사용 시 warning을 출력한다.
- inline secret 자체를 막지는 않는다.
```

---

# 8. 공통 메시지 모델

내부 공통 모델은 다음 개념을 가진다.

```text
title
body
format
priority
tags
attachments
timestamp
id
```

priority enum:

```text
info
success
warning
error
critical
```

format enum:

```text
text
markdown
```

attachment metadata:

```text
path
name
mime_type
size_bytes
sha256
```

각 provider는 공통 메시지를 자신이 지원하는 형식으로 변환한다.

---

# 9. type별 상세 스펙

## 9.1 telegram

설정:

```toml
[channels.personal]
type = "telegram"
bot_token_env = "NOTIFY_TELEGRAM_BOT_TOKEN"
chat_id_env = "NOTIFY_TELEGRAM_CHAT_ID"
parse_mode = "plain"
```

inline 예:

```toml
[channels.personal]
type = "telegram"
bot_token = "123456:ABC..."
chat_id = "123456789"
parse_mode = "plain"
```

지원:

```text
- title
- body
- priority prefix
- file attachments
- image attachments
```

전송 방식:

```text
- 텍스트만 있으면 sendMessage
- 이미지 파일은 sendPhoto
- 일반 파일은 sendDocument
- 메시지와 첨부가 모두 있으면 메시지 먼저 전송 후 첨부 전송
```

parse_mode:

```text
plain
html
markdown-v2
```

기본값:

```text
plain
```

이유:

```text
Telegram MarkdownV2 escaping이 까다롭기 때문에 Agent가 생성한 markdown을 그대로 보내면 깨질 가능성이 높다.
```

---

## 9.2 discord-webhook

설정:

```toml
[channels.team]
type = "discord-webhook"
webhook_url_env = "NOTIFY_DISCORD_WEBHOOK_URL"
username = "Agent Notify"
avatar_url = "https://example.com/avatar.png"
allow_mentions = false
```

inline 예:

```toml
[channels.team]
type = "discord-webhook"
webhook_url = "https://discord.com/api/webhooks/..."
```

지원:

```text
- title
- body
- markdown
- priority prefix
- file attachments
```

mention 정책:

```text
allow_mentions = false 기본값
```

`allow_mentions = false`일 때:

```text
- @everyone 비활성화
- @here 비활성화
- role mention 비활성화
- user mention도 기본적으로 허용하지 않음
```

Discord payload에서는 `allowed_mentions: { parse: [] }`를 사용한다.

---

## 9.3 discord-bot

설정:

```toml
[channels.bot_team]
type = "discord-bot"
bot_token_env = "NOTIFY_DISCORD_BOT_TOKEN"
channel_id_env = "NOTIFY_DISCORD_CHANNEL_ID"
allow_mentions = false
```

inline 예:

```toml
[channels.bot_team]
type = "discord-bot"
bot_token = "..."
channel_id = "123456789012345678"
allow_mentions = false
```

지원:

```text
- 특정 Discord channel_id로 메시지 전송
- title
- body
- markdown
- priority prefix
- file attachments
```

v1 범위:

```text
- 단순 메시지 전송만 지원
- thread_id, reply, edit, interaction은 제외
```

후순위 확장:

```text
thread_id
reply_to_message_id
edit_message_id
role mention
user mention
```

기본 mention 정책은 `discord-webhook`과 동일하다.

---

## 9.4 ntfy

설정:

```toml
[channels.phone]
type = "ntfy"
server = "https://ntfy.sh"
topic_env = "NOTIFY_NTFY_TOPIC"
token_env = "NOTIFY_NTFY_TOKEN"
```

inline 예:

```toml
[channels.phone]
type = "ntfy"
server = "https://ntfy.sh"
topic = "my-topic"
token = "..."
```

지원:

```text
- title
- body
- priority
- tags
```

첨부 파일:

```text
v1에서는 ntfy 첨부 파일 업로드를 지원하지 않는다.
첨부 파일이 지정되면 에러 처리한다.
```

이유:

```text
ntfy의 핵심 용도는 가벼운 푸시 알림이다.
파일 첨부까지 통합하려면 동작 방식과 제한을 별도 설계해야 하므로 v1에서는 제외한다.
```

priority 매핑:

```text
info      default
success   default
warning   high
error     high
critical  urgent
```

---

## 9.5 webhook

`webhook`은 임의 URL에 agent-notify 표준 포맷으로 알림을 전송하는 type이다.

설정:

```toml
[channels.automation]
type = "webhook"
url_env = "NOTIFY_WEBHOOK_URL"
auth_header_env = "NOTIFY_WEBHOOK_AUTH_HEADER"
timeout_seconds = 15
```

inline 예:

```toml
[channels.automation]
type = "webhook"
url = "https://example.com/notify"
auth_header = "Bearer secret"
timeout_seconds = 15
```

기본값:

```text
method = POST
timeout_seconds = 15
```

`auth_header`는 그대로 HTTP `Authorization` 헤더에 들어간다.

```http
Authorization: Bearer secret
```

v1에서는 인증 방식을 복잡하게 나누지 않는다.

---

# 10. webhook protocol v1

`webhook` type은 임의 JSON을 보내는 것이 아니라 **agent-notify webhook protocol v1**을 따른다.

## 10.1 첨부 없는 요청

HTTP:

```http
POST /notify
Content-Type: application/json
Authorization: Bearer ...
```

Body:

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

## 10.2 첨부 있는 요청

HTTP:

```http
POST /notify
Content-Type: multipart/form-data
Authorization: Bearer ...
```

Parts:

```text
payload: application/json
file0: application/octet-stream or detected MIME
file1: application/octet-stream or detected MIME
```

`payload`:

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

## 10.3 webhook 응답 포맷

성공:

```json
{
  "ok": true,
  "id": "server-id-or-echo",
  "message": "accepted"
}
```

경고 포함 성공:

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

실패:

```json
{
  "ok": false,
  "error": {
    "code": "INVALID_PAYLOAD",
    "message": "message.title is required"
  }
}
```

처리 규칙:

```text
- HTTP 2xx + ok=true면 성공
- HTTP 2xx + ok=false면 실패
- HTTP non-2xx면 실패
- 응답이 JSON이 아니면 HTTP status 기준으로 판단한다.
  v1 CLI 출력에는 warning 전용 필드가 없으므로 별도 warning은 출력하지 않는다.
```

---

# 11. file-log

`file-log`는 테스트, 로컬 검증, 자동화 기록 보존용 type이다.

desktop notification 대신 v1의 로컬 테스트 타입으로 사용한다.

## 11.1 설정

```toml
[channels.local]
type = "file-log"
path = "./notify-log"
```

또는 절대 경로:

```toml
[channels.local]
type = "file-log"
path = "/var/log/agent-notify"
```

## 11.2 저장 구조

`path` 아래에 JSONL 로그 파일과 첨부 파일 디렉터리를 만든다.

```text
notify-log/
  notifications.jsonl
  attachments/
    2026-04/
      01J00000000000000000000000/
        chart.png
        report.md
```

## 11.3 JSONL 레코드

`notifications.jsonl`에는 알림 1건당 JSON 한 줄을 기록한다.

예:

```json
{"version":"1","id":"01J00000000000000000000000","timestamp":"2026-04-25T12:34:56Z","channel":"local","type":"file-log","message":{"title":"Chart ready","body":"Attached chart image.","format":"text","priority":"info","tags":["chart"]},"attachments":[{"name":"chart.png","original_path":"./chart.png","stored_path":"attachments/2026-04/01J00000000000000000000000/chart.png","mime_type":"image/png","size_bytes":184203,"sha256":"..."}]}
```

## 11.4 첨부 파일 처리

규칙:

```text
- --file로 지정된 파일은 file-log path 하위 attachments 디렉터리에 복사한다.
- JSONL 로그에는 원본 경로와 저장된 상대 경로를 기록한다.
- 동일 파일명이 충돌하면 안전하게 이름을 변경한다.
```

파일명 충돌 예:

```text
chart.png
chart-1.png
chart-2.png
```

## 11.5 용도

```text
- 로컬 테스트
- Agent 알림 동작 검증
- webhook 수신 서버 없이 payload 확인
- CI에서 알림 기록 보존
- CI에서 dry-run 출력 확인
- 실제 전송 전 메시지와 첨부 산출물 확인
```

`file-log`는 외부 네트워크를 사용하지 않는다.

---

# 12. dry-run

`--dry-run`은 실제 전송이나 파일 복사를 하지 않는다.

예:

```bash
notify send \
  --channel local \
  --title "Chart ready" \
  --body "Attached chart." \
  --file ./chart.png \
  --dry-run
```

출력 예:

```text
Dry run: no notification was sent.

Channel: local
Type: file-log
Title: Chart ready
Priority: info
Format: text
Body length: 15
Attachments:
- ./chart.png
```

`--json`과 함께 사용 시:

```json
{
  "ok": true,
  "dry_run": true,
  "channel": "local",
  "type": "file-log",
  "message": {
    "title": "Chart ready",
    "body": "Attached chart.",
    "format": "text",
    "priority": "info",
    "tags": []
  },
  "attachments": [
    {
      "path": "./chart.png"
    }
  ]
}
```

---

# 13. 안전 정책

사용자가 지적한 대로 v1에서는 과한 보안 옵션을 두지 않는다.

제거하는 설정:

```toml
[security]
max_file_size_mb = 20
scan_text_attachments = true
block_suspicious_attachments = true
warn_inline_secrets = true
```

v1에서는 위와 같은 전역 보안 설정을 제공하지 않는다.

대신 기본 철학은 다음이다.

```text
- 사용자가 지정한 메시지와 파일은 사용자의 책임으로 전송한다.
- 도구는 secret을 CLI 인자로 받지 않는다.
- env 참조 방식을 권장한다.
- inline secret은 허용한다.
- Agent Skill 문서에서는 안전한 사용 관례를 안내한다.
- Discord mass mention은 기본 비활성화한다.
```

즉, 파일명 스캔, 본문 secret scan, 첨부 차단 같은 기능은 v1 범위에서 제외한다.

다만 다음은 도구의 안정성을 위해 필요하다.

```text
- 존재하지 않는 첨부 파일은 에러
- 디렉터리를 --file로 넘기면 에러
- provider가 첨부를 지원하지 않는데 파일이 있으면 에러
- Discord mention은 기본적으로 allowed_mentions 비활성화
```

이건 “보안 정책”이라기보다 “명확한 동작 보장”에 가깝다.

---

# 14. Agent Skill 설계

Skill은 Agent가 안전하고 일관되게 `notify`를 쓰게 하는 문서다.

위치:

```text
skills/notification/SKILL.md
skills/notification/examples.md
```

## 14.1 SKILL.md 초안

---
name: notification
description: Send status notifications from automation agents through configured channels using the notify CLI. Use for completion, failure, warning, approval-needed, or artifact-ready alerts.
---

# Notification Skill

Use `notify` to send concise notifications to configured channels.

## Core command

```bash
notify send --channel <name> --title "<title>" --body "<message>"
```

## Rules

* Use configured channel names.
* Do not pass webhook URLs, bot tokens, API keys, or passwords in commands.
* Prefer short, actionable messages.
* Use `--dry-run` before sending files when the content or destination is uncertain.
* Do not send private source code, credentials, databases, or large logs unless the user explicitly asked.
* Avoid mass mentions such as `@everyone` or `@here`.

## Examples

Task completed:

```bash
notify send --channel personal --title "Task completed" --body "The report was generated successfully."
```

Task failed:

```bash
notify send --channel personal --priority error --title "Task failed" --body "The build failed. See the generated summary."
```

With attachment:

```bash
notify send --channel personal --title "Chart ready" --body "Attached chart image." --file ./chart.png
```

Dry run:

```bash
notify send --channel personal --title "Chart ready" --body "Attached chart image." --file ./chart.png --dry-run
```

중요: Skill에서는 inline secret 사용법을 가르치지 않는다. inline 설정은 README의 “Quick setup”에서만 설명한다.

---

# 15. README 구성

README는 다음 구조가 좋다.

```text
1. What is agent-notify?
2. Installation
3. Quick start
4. Concepts: channel and type
5. Configuration
6. Channel types
   - telegram
   - discord-webhook
   - discord-bot
   - ntfy
   - webhook
   - file-log
7. Webhook protocol v1
8. Agent Skill usage
9. Examples
```

## 15.1 Quick setup

간단 사용자를 위한 inline secret 예:

```toml
default_channel = "team"

[channels.team]
type = "discord-webhook"
webhook_url = "https://discord.com/api/webhooks/..."
```

```bash
notify send --title "Hello" --body "Hello from agent-notify."
```

## 15.2 Recommended setup

권장 방식:

```toml
default_channel = "team"

[channels.team]
type = "discord-webhook"
webhook_url_env = "NOTIFY_DISCORD_WEBHOOK_URL"
```

```bash
export NOTIFY_DISCORD_WEBHOOK_URL="https://discord.com/api/webhooks/..."
notify send --title "Hello" --body "Hello from agent-notify."
```

---

# 16. Rust 설계

## 16.1 Workspace 구조

```text
agent-notify/
  Cargo.toml
  crates/
    notify-cli/
      src/
        main.rs
    notify-core/
      src/
        lib.rs
        config.rs
        message.rs
        provider.rs
        error.rs
        webhook_protocol.rs
        providers/
          mod.rs
          telegram.rs
          discord_webhook.rs
          discord_bot.rs
          ntfy.rs
          webhook.rs
          file_log.rs
  skills/
    notification/
      SKILL.md
      examples.md
  examples/
    notify.toml
    notify.env.example
  docs/
    webhook-v1.md
```

## 16.2 주요 crate 후보

```toml
anyhow = "1"
thiserror = "2"
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json", "multipart", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
clap = { version = "4", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = "0.3"
mime_guess = "2"
sha2 = "0.10"
hostname = "0.4"
time = { version = "0.3", features = ["formatting", "macros"] }
ulid = "1"
```

`ulid`는 알림 ID 생성에 사용한다.

---

# 17. Provider trait

개념적으로 다음 인터페이스를 둔다.

```rust
#[async_trait::async_trait]
pub trait NotifyProvider: Send + Sync {
    fn type_name(&self) -> &'static str;

    fn capabilities(&self) -> ProviderCapabilities;

    async fn check(&self) -> Result<ProviderStatus, NotifyError>;

    async fn send(&self, message: NotifyMessage) -> Result<SendResult, NotifyError>;
}
```

capability 예:

```rust
pub struct ProviderCapabilities {
    pub supports_markdown: bool,
    pub supports_attachments: bool,
    pub supports_multiple_attachments: bool,
}
```

v1에서는 capability를 사용자에게 자세히 노출하지 않아도 된다. 내부 검증용으로 사용한다.

---

# 18. 설정 모델

TOML의 `type` 필드로 enum deserialize한다.

개념 모델:

```rust
#[derive(Debug, Deserialize)]
pub struct Config {
    pub default_channel: Option<String>,
    pub channels: BTreeMap<String, ChannelConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ChannelConfig {
    Telegram(TelegramConfig),
    DiscordWebhook(DiscordWebhookConfig),
    DiscordBot(DiscordBotConfig),
    Ntfy(NtfyConfig),
    Webhook(WebhookConfig),
    FileLog(FileLogConfig),
}
```

secret-like field는 다음 공통 타입을 사용한다.

```rust
pub enum SecretRef {
    Inline(String),
    Env(String),
}
```

TOML에서는 `bot_token` / `bot_token_env`처럼 받고, config validation 단계에서 `SecretRef`로 정규화한다.

---

# 19. 결과 출력

일반 출력:

```text
Sent notification.

Channel: personal
Type: telegram
Title: Task completed
Attachments: 0
```

JSON 출력:

```json
{
  "ok": true,
  "channel": "personal",
  "type": "telegram",
  "id": "01J00000000000000000000000",
  "sent": true,
  "dry_run": false,
  "attachments": []
}
```

실패 출력:

```text
Error: channel "personal" is missing environment variable NOTIFY_TELEGRAM_BOT_TOKEN
```

JSON 실패:

```json
{
  "ok": false,
  "error": {
    "code": "MISSING_ENV",
    "message": "channel \"personal\" is missing environment variable NOTIFY_TELEGRAM_BOT_TOKEN"
  }
}
```

---

# 20. v1 범위 확정

## 포함

```text
Rust CLI
TOML config
channel/type 모델
notify send
notify channels
notify check
notify test
telegram
discord-webhook
discord-bot
ntfy
webhook
file-log
webhook protocol v1
Agent Skill 문서
README
examples/notify.toml
examples/notify.env.example
```

## 제외

```text
email
gotify
slack
teams
mattermost
desktop notification
stdout provider
secret scanning
suspicious attachment blocking
max file size policy
interactive confirmation
message scheduling
retry queue
daemon mode
encrypted config
OS keychain
```

---

# 21. 구현 우선순위

```text
1. Config model
2. Common message model
3. file-log provider
4. CLI send/channels/check/test
5. webhook provider + webhook protocol v1
6. ntfy provider
7. discord-webhook provider
8. telegram provider
9. discord-bot provider
10. Agent Skill 문서
11. README/examples 정리
```

`file-log`를 먼저 구현하면 외부 서비스 없이 CLI 동작과 메시지 모델을 검증할 수 있다.

---

# 22. 최종 방향성

이 프로젝트의 핵심은 다음 한 문장으로 정리된다.

```text
agent-notify는 AI Agent와 자동화 스크립트가 channel 이름만 지정해서 여러 알림 경로로 메시지와 첨부 파일을 보낼 수 있게 해주는 Rust 기반 통합 알림 CLI다.
```

최종 v1 설계는 다음이다.

```text
channel:
  사용자가 선택하는 논리적 목적지 이름

type:
  telegram | discord-webhook | discord-bot | ntfy | webhook | file-log

secret:
  *_env 권장
  inline도 허용
  CLI 인자 secret은 미지원

attachments:
  지원 provider에서는 전송
  file-log에서는 attachments 하위 폴더에 복사 후 JSONL에서 경로 참조
  미지원 provider에서는 에러

webhook:
  agent-notify webhook protocol v1 사용
  JSON 또는 multipart/form-data

Agent Skill:
  안전한 CLI 사용법만 설명
  inline secret은 README에서만 설명
```
