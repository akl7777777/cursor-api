use crate::{
    app::{
        constant::{
            CONTENT_TYPE_TEXT_HTML_WITH_UTF8, CONTENT_TYPE_TEXT_PLAIN_WITH_UTF8, ROUTE_TOKENS_PATH,
        },
        model::{AppConfig, PageContent},
    },
    common::{
        model::ApiStatus,
        utils::{extract_time, extract_time_ks, extract_user_id, validate_token_and_checksum},
    },
};
use axum::{
    Json,
    body::Body,
    http::header::CONTENT_TYPE,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

pub async fn handle_tokens_page() -> impl IntoResponse {
    match AppConfig::get_page_content(ROUTE_TOKENS_PATH).unwrap_or_default() {
        PageContent::Default => Response::builder()
            .header(CONTENT_TYPE, CONTENT_TYPE_TEXT_HTML_WITH_UTF8)
            .body(Body::from(include_str!("../../../static/tokens.min.html")))
            .unwrap(),
        PageContent::Text(content) => Response::builder()
            .header(CONTENT_TYPE, CONTENT_TYPE_TEXT_PLAIN_WITH_UTF8)
            .body(Body::from(content))
            .unwrap(),
        PageContent::Html(content) => Response::builder()
            .header(CONTENT_TYPE, CONTENT_TYPE_TEXT_HTML_WITH_UTF8)
            .body(Body::from(content))
            .unwrap(),
    }
}

#[derive(Deserialize)]
pub struct TokenRequest {
    pub token: Option<String>,
}

#[derive(Serialize)]
pub struct BasicCalibrationResponse {
    pub status: ApiStatus,
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum_time: Option<u64>,
}

pub async fn handle_basic_calibration(
    Json(request): Json<TokenRequest>,
) -> Json<BasicCalibrationResponse> {
    // 从请求头中获取并验证 auth token
    let auth_token = match request.token {
        Some(token) => token,
        None => {
            return Json(BasicCalibrationResponse {
                status: ApiStatus::Error,
                message: Some("未提供授权令牌".to_string()),
                user_id: None,
                create_at: None,
                checksum_time: None,
            });
        }
    };

    // 校验 token 和 checksum
    let (token, checksum) = match validate_token_and_checksum(&auth_token) {
        Some(parts) => parts,
        None => {
            return Json(BasicCalibrationResponse {
                status: ApiStatus::Error,
                message: Some("无效令牌或无效校验和".to_string()),
                user_id: None,
                create_at: None,
                checksum_time: None,
            });
        }
    };

    // 提取用户ID和创建时间
    let user_id = extract_user_id(&token);
    let create_at = extract_time(&token).map(|dt| dt.to_string());
    let checksum_time = extract_time_ks(&checksum[..8]);

    // 返回校验结果
    Json(BasicCalibrationResponse {
        status: ApiStatus::Success,
        message: Some("校验成功".to_string()),
        user_id,
        create_at,
        checksum_time,
    })
}
