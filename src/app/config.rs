use super::{constant::AUTHORIZATION_BEARER_PREFIX, lazy::AUTH_TOKEN, model::AppConfig};
use crate::common::model::{
    ApiStatus, ErrorResponse, NormalResponse,
    config::{ConfigData, ConfigUpdateRequest},
};
use axum::{
    Json,
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
};

// 定义处理更新操作的宏
macro_rules! handle_updates {
    ($request:expr, $($field:ident => $update_fn:expr),* $(,)?) => {
        $(
            if let Some(value) = $request.$field {
                $update_fn(value);
            }
        )*
    };
}

// 定义处理重置操作的宏
macro_rules! handle_resets {
    ($request:expr, $($field:ident => $reset_fn:expr),* $(,)?) => {
        $(
            if $request.$field.is_some() {
                $reset_fn();
            }
        )*
    };
}

pub async fn handle_config_update(
    headers: HeaderMap,
    Json(request): Json<ConfigUpdateRequest>,
) -> Result<Json<NormalResponse<ConfigData>>, (StatusCode, Json<ErrorResponse>)> {
    let auth_header = headers
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix(AUTHORIZATION_BEARER_PREFIX))
        .ok_or((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                status: ApiStatus::Failure,
                code: Some(401),
                error: Some("未提供认证令牌".to_string()),
                message: None,
            }),
        ))?;

    if auth_header != AUTH_TOKEN.as_str() {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                status: ApiStatus::Failure,
                code: Some(401),
                error: Some("无效的认证令牌".to_string()),
                message: None,
            }),
        ));
    }

    match request.action.as_str() {
        "get" => Ok(Json(NormalResponse {
            status: ApiStatus::Success,
            data: Some(ConfigData {
                page_content: AppConfig::get_page_content(&request.path),
                vision_ability: AppConfig::get_vision_ability(),
                enable_slow_pool: AppConfig::get_slow_pool(),
                enable_all_claude: AppConfig::get_allow_claude(),
                usage_check_models: AppConfig::get_usage_check(),
                enable_dynamic_key: AppConfig::get_dynamic_key(),
                share_token: AppConfig::get_share_token(),
                proxies: AppConfig::get_proxies(),
                include_web_references: AppConfig::get_web_refs(),
            }),
            message: None,
        })),

        "update" => {
            // 处理页面内容更新
            if !request.path.is_empty() && request.content.is_some() {
                let content = request.content.unwrap();
                if let Err(e) = AppConfig::update_page_content(&request.path, content) {
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            status: ApiStatus::Failure,
                            code: Some(500),
                            error: Some(format!("更新页面内容失败: {}", e)),
                            message: None,
                        }),
                    ));
                }
            }

            handle_updates!(request,
                vision_ability => AppConfig::update_vision_ability,
                enable_slow_pool => AppConfig::update_slow_pool,
                enable_all_claude => AppConfig::update_allow_claude,
                usage_check_models => AppConfig::update_usage_check,
                enable_dynamic_key => AppConfig::update_dynamic_key,
                share_token => AppConfig::update_share_token,
                proxies => AppConfig::update_proxies,
                include_web_references => AppConfig::update_web_refs,
            );

            Ok(Json(NormalResponse {
                status: ApiStatus::Success,
                data: None,
                message: Some("配置已更新".to_string()),
            }))
        }

        "reset" => {
            // 重置页面内容
            if !request.path.is_empty() {
                if let Err(e) = AppConfig::reset_page_content(&request.path) {
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            status: ApiStatus::Failure,
                            code: Some(500),
                            error: Some(format!("重置页面内容失败: {}", e)),
                            message: None,
                        }),
                    ));
                }
            }

            handle_resets!(request,
                vision_ability => AppConfig::reset_vision_ability,
                enable_slow_pool => AppConfig::reset_slow_pool,
                enable_all_claude => AppConfig::reset_allow_claude,
                usage_check_models => AppConfig::reset_usage_check,
                enable_dynamic_key => AppConfig::reset_dynamic_key,
                share_token => AppConfig::reset_share_token,
                proxies => AppConfig::reset_proxies,
                include_web_references => AppConfig::reset_web_refs,
            );

            Ok(Json(NormalResponse {
                status: ApiStatus::Success,
                data: None,
                message: Some("配置已重置".to_string()),
            }))
        }

        _ => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                status: ApiStatus::Failure,
                code: Some(400),
                error: Some("无效的操作类型".to_string()),
                message: None,
            }),
        )),
    }
}
