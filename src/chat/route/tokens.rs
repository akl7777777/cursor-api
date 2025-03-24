use crate::{
    app::{
        constant::AUTHORIZATION_BEARER_PREFIX,
        lazy::AUTH_TOKEN,
        model::{
            AppState, TokenAddRequest, TokenInfo, TokenInfoResponse, TokenManager,
            TokenTagsResponse, TokenTagsUpdateRequest, TokenUpdateRequest, TokensDeleteRequest,
            TokensDeleteResponse,
        },
    },
    common::{
        model::{ApiStatus, ErrorResponse, error::ChatError, userinfo::TokenProfile},
        utils::{
            generate_checksum_with_default, generate_checksum_with_repair,
            load_tokens_from_content, parse_token, validate_token,
        },
    },
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

pub async fn handle_get_tokens(
    State(state): State<Arc<Mutex<AppState>>>,
    headers: HeaderMap,
) -> Result<Json<TokenInfoResponse>, StatusCode> {
    // 验证 AUTH_TOKEN
    let auth_header = headers
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix(AUTHORIZATION_BEARER_PREFIX))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if auth_header != AUTH_TOKEN.as_str() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let state = state.lock().await;
    let tokens = state.token_manager.tokens.clone();
    let tokens_count = tokens.len();

    Ok(Json(TokenInfoResponse {
        status: ApiStatus::Success,
        tokens: Some(tokens),
        tokens_count,
        message: None,
    }))
}

pub async fn handle_update_tokens(
    State(state): State<Arc<Mutex<AppState>>>,
    headers: HeaderMap,
    Json(request): Json<TokenUpdateRequest>,
) -> Result<Json<TokenInfoResponse>, StatusCode> {
    // 验证 AUTH_TOKEN
    let auth_header = headers
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix(AUTHORIZATION_BEARER_PREFIX))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if auth_header != AUTH_TOKEN.as_str() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // 获取当前的 token_manager 以保留现有 token 的 profile 和 tags
    let current_token_manager = {
        let state = state.lock().await;
        state.token_manager.clone()
    };

    // 创建 token -> (profile, tags) 映射
    let token_info_map: HashMap<String, (Option<TokenProfile>, Option<Vec<String>>)> =
        current_token_manager
            .tokens
            .iter()
            .map(|token| {
                (
                    token.token.clone(),
                    (token.profile.clone(), token.tags.clone()),
                )
            })
            .collect();

    // 从请求内容加载新的 tokens
    let mut new_tokens = load_tokens_from_content(&request.tokens);

    // 为相同的 token 保留原有的 profile 和 tags
    for token_info in &mut new_tokens {
        if let Some((profile, tags)) = token_info_map.get(&token_info.token) {
            token_info.profile = profile.clone();
            token_info.tags = tags.clone();
        }
    }

    // 创建新的 TokenManager
    let token_manager = TokenManager::new(new_tokens);
    let tokens_count = token_manager.tokens.len();

    // 保存到文件
    token_manager
        .save_tokens()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // 更新应用状态
    {
        let mut state = state.lock().await;
        state.token_manager = token_manager;
    }

    Ok(Json(TokenInfoResponse {
        status: ApiStatus::Success,
        tokens: None,
        tokens_count,
        message: Some("Token files have been updated and reloaded".to_string()),
    }))
}

pub async fn handle_add_tokens(
    State(state): State<Arc<Mutex<AppState>>>,
    headers: HeaderMap,
    Json(request): Json<TokenAddRequest>,
) -> Result<Json<TokenInfoResponse>, (StatusCode, Json<ErrorResponse>)> {
    // 验证 AUTH_TOKEN
    let auth_header = headers
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix(AUTHORIZATION_BEARER_PREFIX))
        .ok_or((
            StatusCode::UNAUTHORIZED,
            Json(ChatError::Unauthorized.to_json()),
        ))?;

    if auth_header != AUTH_TOKEN.as_str() {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ChatError::Unauthorized.to_json()),
        ));
    }

    // 获取当前的 token_manager
    let mut token_manager = {
        let state = state.lock().await;
        state.token_manager.clone()
    };

    // 创建现有token的集合
    let existing_tokens: std::collections::HashSet<_> = token_manager
        .tokens
        .iter()
        .map(|info| info.token.as_str())
        .collect();

    // 处理新的tokens
    let mut new_tokens = Vec::with_capacity(request.tokens.len());
    for token_info in request.tokens {
        let parsed_token = parse_token(&token_info.token);
        if !existing_tokens.contains(parsed_token.as_str()) && validate_token(&parsed_token) {
            new_tokens.push(TokenInfo {
                token: parsed_token,
                checksum: token_info
                    .checksum
                    .as_deref()
                    .map(generate_checksum_with_repair)
                    .unwrap_or_else(generate_checksum_with_default),
                profile: None,
                tags: request.tags.clone(),
            });
        }
    }

    // 如果有新tokens才进行后续操作
    if !new_tokens.is_empty() {
        // 添加新tokens
        token_manager.tokens.extend(new_tokens);
        let tokens_count = token_manager.tokens.len();

        // 更新全局标签
        if let Some(ref tags) = request.tags {
            token_manager.update_global_tags(tags);
        }

        // 保存到文件
        token_manager.save_tokens().await.map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    status: ApiStatus::Error,
                    code: None,
                    error: Some("Failed to save token list".to_string()),
                    message: Some("无法保存token list".to_string()),
                }),
            )
        })?;

        // 更新应用状态
        {
            let mut state = state.lock().await;
            state.token_manager = token_manager;
        }

        Ok(Json(TokenInfoResponse {
            status: ApiStatus::Success,
            tokens: None,
            tokens_count,
            message: Some("New tokens have been added and reloaded".to_string()),
        }))
    } else {
        // 如果没有新tokens，返回当前状态
        let tokens = token_manager.tokens.clone();
        let tokens_count = tokens.len();

        Ok(Json(TokenInfoResponse {
            status: ApiStatus::Success,
            tokens: Some(tokens),
            tokens_count,
            message: Some("No new tokens were added".to_string()),
        }))
    }
}

pub async fn handle_delete_tokens(
    State(state): State<Arc<Mutex<AppState>>>,
    headers: HeaderMap,
    Json(request): Json<TokensDeleteRequest>,
) -> Result<Json<TokensDeleteResponse>, (StatusCode, Json<ErrorResponse>)> {
    // 验证 AUTH_TOKEN
    let auth_header = headers
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix(AUTHORIZATION_BEARER_PREFIX))
        .ok_or((
            StatusCode::UNAUTHORIZED,
            Json(ChatError::Unauthorized.to_json()),
        ))?;

    if auth_header != AUTH_TOKEN.as_str() {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ChatError::Unauthorized.to_json()),
        ));
    }

    // 获取当前的 token_manager
    let mut token_manager = {
        let state = state.lock().await;
        state.token_manager.clone()
    };

    // 创建要删除的tokens的HashSet，提高查找效率
    let tokens_to_delete: std::collections::HashSet<_> = request.tokens.iter().collect();

    // 如果需要的话计算 failed_tokens
    let failed_tokens = if request.expectation.needs_failed_tokens() {
        Some(
            request
                .tokens
                .iter()
                .filter(|token| {
                    !token_manager
                        .tokens
                        .iter()
                        .any(|token_info| token_info.token == **token)
                })
                .cloned()
                .collect::<Vec<String>>(),
        )
    } else {
        None
    };

    let original_count: usize = token_manager.tokens.len();

    // 从每个分组中删除指定的tokens
    token_manager
        .tokens
        .retain(|token_info| !tokens_to_delete.contains(&token_info.token));

    let new_count: usize = token_manager.tokens.len();

    // 如果有tokens被删除才进行更新操作
    if new_count < original_count {
        // 保存到文件
        token_manager.save_tokens().await.map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    status: ApiStatus::Error,
                    code: None,
                    error: Some("Failed to save token list".to_string()),
                    message: Some("无法保存token list".to_string()),
                }),
            )
        })?;

        // 如果需要的话计算 updated_tokens
        let updated_tokens = if request.expectation.needs_updated_tokens() {
            Some(
                token_manager
                    .tokens
                    .iter()
                    .map(|t| t.token.clone())
                    .collect(),
            )
        } else {
            None
        };

        // 更新状态
        {
            let mut state = state.lock().await;
            state.token_manager = token_manager;
        }

        Ok(Json(TokensDeleteResponse {
            status: ApiStatus::Success,
            updated_tokens,
            failed_tokens,
        }))
    } else {
        // 如果没有tokens被删除
        Ok(Json(TokensDeleteResponse {
            status: ApiStatus::Success,
            updated_tokens: if request.expectation.needs_updated_tokens() {
                Some(
                    token_manager
                        .tokens
                        .iter()
                        .map(|t| t.token.clone())
                        .collect(),
                )
            } else {
                None
            },
            failed_tokens,
        }))
    }
}

pub async fn handle_update_token_tags(
    State(state): State<Arc<Mutex<AppState>>>,
    headers: HeaderMap,
    Json(request): Json<TokenTagsUpdateRequest>,
) -> Result<Json<TokenTagsResponse>, (StatusCode, Json<ErrorResponse>)> {
    // 验证 AUTH_TOKEN
    let auth_header = headers
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix(AUTHORIZATION_BEARER_PREFIX))
        .ok_or((
            StatusCode::UNAUTHORIZED,
            Json(ChatError::Unauthorized.to_json()),
        ))?;

    if auth_header != AUTH_TOKEN.as_str() {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ChatError::Unauthorized.to_json()),
        ));
    }

    // 获取并更新 token_manager
    {
        let mut state = state.lock().await;
        if let Err(e) = state
            .token_manager
            .update_tokens_tags(request.tokens, request.tags)
        {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    status: ApiStatus::Error,
                    code: None,
                    error: Some(e.to_string()),
                    message: Some("更新标签失败".to_string()),
                }),
            ));
        }

        // 保存更改
        if (state.token_manager.save_tokens().await).is_err() {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    status: ApiStatus::Error,
                    code: None,
                    error: Some("Failed to save token tags".to_string()),
                    message: Some("无法保存标签信息".to_string()),
                }),
            ));
        }
    }

    Ok(Json(TokenTagsResponse {
        status: ApiStatus::Success,
        message: Some("标签更新成功".to_string()),
    }))
}
