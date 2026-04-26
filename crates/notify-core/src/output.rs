use serde::Serialize;

use crate::{MessageFormat, NotifyError, Priority, provider::StoredAttachment};

#[derive(Debug, Serialize)]
pub struct SendOutput {
    pub ok: bool,
    pub sent: bool,
    pub dry_run: bool,
    pub results: Vec<SendResultOutput>,
}

#[derive(Debug, Serialize)]
pub struct SendResultOutput {
    pub ok: bool,
    pub channel: String,
    #[serde(rename = "type")]
    pub channel_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub sent: bool,
    pub dry_run: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<StoredAttachment>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBody>,
}

#[derive(Debug, Serialize)]
pub struct DryRunOutput {
    pub ok: bool,
    pub dry_run: bool,
    pub sent: bool,
    pub results: Vec<DryRunResultOutput>,
}

#[derive(Debug, Serialize)]
pub struct DryRunResultOutput {
    pub ok: bool,
    pub channel: String,
    #[serde(rename = "type")]
    pub channel_type: String,
    pub sent: bool,
    pub dry_run: bool,
    pub message: DryRunMessage,
    pub attachments: Vec<DryRunAttachment>,
}

#[derive(Debug, Serialize)]
pub struct DryRunMessage {
    pub title: String,
    pub body: Option<String>,
    pub format: MessageFormat,
    pub priority: Priority,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DryRunAttachment {
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorOutput {
    pub ok: bool,
    pub error: ErrorBody,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}

impl From<&NotifyError> for ErrorOutput {
    fn from(error: &NotifyError) -> Self {
        Self {
            ok: false,
            error: ErrorBody {
                code: error.code().to_string(),
                message: error.to_string(),
            },
        }
    }
}
