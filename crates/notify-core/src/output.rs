use serde::Serialize;

use crate::{NotifyError, provider::StoredAttachment};

#[derive(Debug, Serialize)]
pub struct SendOutput {
    pub ok: bool,
    pub channel: String,
    #[serde(rename = "type")]
    pub channel_type: String,
    pub id: String,
    pub sent: bool,
    pub dry_run: bool,
    pub attachments: Vec<StoredAttachment>,
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
