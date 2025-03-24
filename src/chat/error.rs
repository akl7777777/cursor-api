use super::aiserver::v1::ErrorDetails;
use crate::common::model::{ApiStatus, ErrorResponse as CommonErrorResponse};
use base64::{Engine as _, engine::general_purpose::STANDARD_NO_PAD};
use prost::Message as _;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct ChatError {
    error: ErrorBody,
}

#[derive(Deserialize)]
pub struct ErrorBody {
    code: String,
    // message: String, always: Error
    details: Vec<ErrorDetail>,
}

#[derive(Deserialize)]
pub struct ErrorDetail {
    // #[serde(rename = "type")]
    // error_type: String, always: aiserver.v1.ErrorDetails
    // debug: ErrorDebug,
    value: String,
}

// #[derive(Deserialize)]
// pub struct ErrorDebug {
//     error: String,
//     details: ErrorDetails,
//     // #[serde(rename = "isExpected")]
//     // is_expected: Option<bool>,
// }

// #[derive(Deserialize)]
// pub struct ErrorDetails {
//     title: String,
//     detail: String,
//     // #[serde(rename = "isRetryable")]
//     // is_retryable: Option<bool>,
// }

impl ChatError {
    pub fn into_error_response(self) -> ErrorResponse {
        if self.error.details.is_empty() {
            return ErrorResponse {
                status: 500,
                code: "unknown".to_string(),
                error: None,
            };
        }

        let error_details = self.error.details.first().and_then(|detail| {
            STANDARD_NO_PAD
                .decode(&detail.value)
                .ok()
                .map(bytes::Bytes::from)
                .and_then(|buf| ErrorDetails::decode(buf).ok())
        });

        let status = error_details
            .as_ref()
            .map(|details| details.status_code())
            .unwrap_or(500);

        ErrorResponse {
            status,
            code: self.error.code,
            error: error_details
                .and_then(|details| details.details)
                .map(|custom_details| Error {
                    message: custom_details.title,
                    details: custom_details.detail,
                }),
        }
    }
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub status: u16,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Error>,
}

#[derive(Serialize)]
pub struct Error {
    pub message: String,
    pub details: String,
    // pub value: String,
}

impl ErrorResponse {
    // pub fn to_json(&self) -> serde_json::Value {
    //     serde_json::to_value(self).unwrap()
    // }

    pub fn status_code(&self) -> StatusCode {
        StatusCode::from_u16(self.status).unwrap()
    }

    pub fn native_code(&self) -> String {
        self.error.as_ref().map_or_else(
            || self.code.replace("_", " "),
            |error| error.message.clone(),
        )
    }

    pub fn into_common(self) -> CommonErrorResponse {
        CommonErrorResponse {
            status: ApiStatus::Error,
            code: Some(self.status),
            error: self
                .error
                .as_ref()
                .map(|error| error.message.clone())
                .or(Some(self.code.clone())),
            message: self.error.as_ref().map(|error| error.details.clone()),
        }
    }
}

pub enum StreamError {
    ChatError(ChatError),
    DataLengthLessThan5,
    EmptyStream,
}

impl std::fmt::Display for StreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StreamError::ChatError(error) => write!(f, "{}", error.error.code),
            StreamError::DataLengthLessThan5 => write!(f, "data length less than 5"),
            StreamError::EmptyStream => write!(f, "empty stream"),
        }
    }
}
