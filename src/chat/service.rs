use crate::{
    app::{
        constant::{
            AUTHORIZATION_BEARER_PREFIX, FINISH_REASON_STOP, OBJECT_CHAT_COMPLETION,
            OBJECT_CHAT_COMPLETION_CHUNK,
        },
        lazy::{
            AUTH_TOKEN, CURSOR_API2_CHAT_URL, CURSOR_API2_CHAT_WEB_URL, IS_UNLIMITED_REQUEST_LOGS,
            KEY_PREFIX, KEY_PREFIX_LEN, REQUEST_LOGS_LIMIT, SERVICE_TIMEOUT,
        },
        model::{
            AppConfig, AppState, ChatRequest, LogStatus, RequestLog, TimingInfo, TokenInfo,
            UsageCheck,
        },
    },
    chat::{
        config::KeyConfig,
        constant::{Models, USAGE_CHECK_MODELS},
        error::StreamError,
        model::{
            ChatResponse, Choice, Delta, Message, MessageContent, ModelsResponse, Role, Usage,
        },
        stream::{StreamDecoder, StreamMessage},
    },
    common::{
        client::build_client,
        model::{ApiStatus, ErrorResponse, error::ChatError, userinfo::MembershipType},
        utils::{
            TrimNewlines as _, format_time_ms, from_base64, get_available_models,
            get_token_profile, tokeninfo_to_token, validate_token_and_checksum,
        },
    },
};
use axum::{
    Json,
    body::Body,
    extract::State,
    http::{
        HeaderMap, StatusCode,
        header::{AUTHORIZATION, CONTENT_TYPE},
    },
    response::Response,
};
use bytes::Bytes;
use futures::StreamExt;
use prost::Message as _;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{
    convert::Infallible,
    sync::{Arc, atomic::AtomicBool},
};
use tokio::sync::Mutex;
use uuid::Uuid;

use super::{constant::LONG_CONTEXT_MODELS, model::Model};

// 辅助函数：提取认证token
fn extract_auth_token(headers: &HeaderMap) -> Result<&str, (StatusCode, Json<ErrorResponse>)> {
    headers
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix(AUTHORIZATION_BEARER_PREFIX))
        .ok_or((
            StatusCode::UNAUTHORIZED,
            Json(ChatError::Unauthorized.to_json()),
        ))
}

// 辅助函数：解析token信息
async fn resolve_token_info(
    auth_header: &str,
    state: &Arc<Mutex<AppState>>,
) -> Result<(String, String), (StatusCode, Json<ErrorResponse>)> {
    match auth_header {
        // 管理员Token处理
        token if is_admin_token(token) => resolve_admin_token(state).await,

        // 动态密钥处理
        token if is_dynamic_key(token) => resolve_dynamic_key(token),

        // 普通用户Token处理
        token => validate_token_and_checksum(token).ok_or((
            StatusCode::UNAUTHORIZED,
            Json(ChatError::Unauthorized.to_json()),
        )),
    }
}

// 辅助函数：检查是否为管理员token
fn is_admin_token(token: &str) -> bool {
    token == AUTH_TOKEN.as_str()
        || (AppConfig::is_share() && token == AppConfig::get_share_token().as_str())
}

// 辅助函数：检查是否为动态密钥
fn is_dynamic_key(token: &str) -> bool {
    AppConfig::get_dynamic_key() && token.starts_with(&*KEY_PREFIX)
}

// 辅助函数：处理管理员token
async fn resolve_admin_token(
    state: &Arc<Mutex<AppState>>,
) -> Result<(String, String), (StatusCode, Json<ErrorResponse>)> {
    static CURRENT_KEY_INDEX: AtomicUsize = AtomicUsize::new(0);

    let state_guard = state.lock().await;
    let token_infos = &state_guard.token_manager.tokens;

    if token_infos.is_empty() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ChatError::NoTokens.to_json()),
        ));
    }

    let index = CURRENT_KEY_INDEX.fetch_add(1, Ordering::SeqCst) % token_infos.len();
    let token_info = &token_infos[index];

    Ok((token_info.token.clone(), token_info.checksum.clone()))
}

// 辅助函数：处理动态密钥
fn resolve_dynamic_key(token: &str) -> Result<(String, String), (StatusCode, Json<ErrorResponse>)> {
    from_base64(&token[*KEY_PREFIX_LEN..])
        .and_then(|decoded_bytes| KeyConfig::decode(&decoded_bytes[..]).ok())
        .and_then(|key_config| key_config.auth_token)
        .and_then(|token_info| tokeninfo_to_token(&token_info))
        .ok_or((
            StatusCode::UNAUTHORIZED,
            Json(ChatError::Unauthorized.to_json()),
        ))
}

// 模型列表处理
pub async fn handle_models(
    State(state): State<Arc<Mutex<AppState>>>,
    headers: HeaderMap,
) -> Result<Json<ModelsResponse>, (StatusCode, Json<ErrorResponse>)> {
    // 如果没有认证头，返回默认可用模型
    if headers.get(AUTHORIZATION).is_none() {
        return Ok(Json(ModelsResponse::with_default_models()));
    }

    // 提取和验证认证token
    let auth_token = extract_auth_token(&headers)?;
    let (token, checksum) = resolve_token_info(auth_token, &state).await?;

    // 获取可用模型列表
    let models = get_available_models(&token, &checksum).await.ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            status: ApiStatus::Failure,
            code: Some(StatusCode::INTERNAL_SERVER_ERROR.as_u16()),
            error: Some("Failed to fetch available models".to_string()),
            message: Some("Unable to get available models".to_string()),
        }),
    ))?;

    // 更新模型列表
    if let Err(e) = Models::update(models) {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                status: ApiStatus::Failure,
                code: Some(StatusCode::INTERNAL_SERVER_ERROR.as_u16()),
                error: Some("Failed to update models".to_string()),
                message: Some(e.to_string()),
            }),
        ));
    }

    Ok(Json(ModelsResponse::new(Models::to_arc())))
}

// 聊天处理函数的签名
pub async fn handle_chat(
    State(state): State<Arc<Mutex<AppState>>>,
    headers: HeaderMap,
    Json(request): Json<ChatRequest>,
) -> Result<Response<Body>, (StatusCode, Json<ErrorResponse>)> {
    let allow_claude = AppConfig::get_allow_claude();

    let is_search = request.model.ends_with("-online");
    let model_name = if is_search {
        request.model[..request.model.len() - 7].to_string()
    } else {
        request.model.clone()
    };

    // 验证模型是否支持并获取模型信息
    let model =
        if Models::exists(&model_name) || (allow_claude && request.model.starts_with("claude")) {
            Some(&model_name)
        } else {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ChatError::ModelNotSupported(request.model).to_json()),
            ));
        };

    let request_time = chrono::Local::now();

    // 验证请求
    if request.messages.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ChatError::EmptyMessages.to_json()),
        ));
    }

    // 获取并处理认证令牌
    let auth_header = headers
        .get(AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix(AUTHORIZATION_BEARER_PREFIX))
        .ok_or((
            StatusCode::UNAUTHORIZED,
            Json(ChatError::Unauthorized.to_json()),
        ))?;

    let mut current_config = KeyConfig::new_with_global();

    // 验证认证token并获取token信息
    let (auth_token, checksum) = match auth_header {
        // 管理员Token验证逻辑
        token
            if token == AUTH_TOKEN.as_str()
                || (AppConfig::is_share() && token == AppConfig::get_share_token().as_str()) =>
        {
            static CURRENT_KEY_INDEX: AtomicUsize = AtomicUsize::new(0);
            let state_guard = state.lock().await;
            let token_infos = &state_guard.token_manager.tokens;

            // 检查是否存在可用的token
            if token_infos.is_empty() {
                return Err((
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(ChatError::NoTokens.to_json()),
                ));
            }

            // 轮询选择token
            let index = CURRENT_KEY_INDEX.fetch_add(1, Ordering::SeqCst) % token_infos.len();
            let token_info = &token_infos[index];
            (token_info.token.clone(), token_info.checksum.clone())
        }

        token if AppConfig::get_dynamic_key() && token.starts_with(&*KEY_PREFIX) => {
            from_base64(&token[*KEY_PREFIX_LEN..])
                .and_then(|decoded_bytes| KeyConfig::decode(&decoded_bytes[..]).ok())
                .and_then(|key_config| {
                    key_config.copy_without_auth_token(&mut current_config);
                    key_config.auth_token
                })
                .and_then(|token_info| tokeninfo_to_token(&token_info))
                .ok_or((
                    StatusCode::UNAUTHORIZED,
                    Json(ChatError::Unauthorized.to_json()),
                ))?
        }

        // 普通用户Token验证逻辑
        token => validate_token_and_checksum(token).ok_or((
            StatusCode::UNAUTHORIZED,
            Json(ChatError::Unauthorized.to_json()),
        ))?,
    };

    let current_config = current_config;

    let current_id: u64;

    // 更新请求日志
    {
        let state_clone = state.clone();
        let mut state = state.lock().await;
        state.request_manager.total_requests += 1;
        state.request_manager.active_requests += 1;

        let mut found_count: u32 = 0;
        let mut no_prompt_count: u32 = 0;
        let mut need_profile_check = false;

        for log in state.request_manager.request_logs.iter().rev() {
            if log.token_info.token == auth_token {
                if !LONG_CONTEXT_MODELS.contains(&log.model.as_str()) {
                    found_count += 1;
                }

                if log.prompt.is_none() {
                    no_prompt_count += 1;
                }

                if found_count == 1 && log.token_info.profile.is_some() {
                    if let Some(profile) = &log.token_info.profile {
                        if profile.stripe.membership_type == MembershipType::Free {
                            let is_premium = USAGE_CHECK_MODELS.contains(&model_name.as_str());
                            need_profile_check =
                                if is_premium {
                                    profile.usage.premium.max_requests.is_some_and(|max| {
                                        profile.usage.premium.num_requests >= max
                                    })
                                } else {
                                    profile.usage.standard.max_requests.is_some_and(|max| {
                                        profile.usage.standard.num_requests >= max
                                    })
                                };
                        }
                    }
                }

                if found_count == 2 {
                    break;
                }
            }
        }

        if found_count == 2 && no_prompt_count == 2 {
            state.request_manager.active_requests -= 1;
            state.request_manager.error_requests += 1;
            return Err((
                StatusCode::TOO_MANY_REQUESTS,
                Json(ErrorResponse {
                    status: ApiStatus::Error,
                    code: Some(429),
                    error: Some("rate_limit_exceeded".to_string()),
                    message: Some("Too many requests without prompt".to_string()),
                }),
            ));
        }

        // 处理检查结果
        if need_profile_check {
            state.request_manager.active_requests -= 1;
            state.request_manager.error_requests += 1;
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(ChatError::Unauthorized.to_json()),
            ));
        }

        let next_id = state
            .request_manager
            .request_logs
            .last()
            .map_or(1, |log| log.id + 1);
        current_id = next_id;

        // 如果需要获取用户使用情况,创建后台任务获取profile
        if model
            .map(|m| {
                Model::is_usage_check(
                    m,
                    UsageCheck::from_proto(current_config.usage_check_models.as_ref()),
                )
            })
            .unwrap_or(false)
        {
            let auth_token_clone = auth_token.clone();
            let state_clone = state_clone.clone();
            let log_id = next_id;

            tokio::spawn(async move {
                let profile = get_token_profile(&auth_token_clone).await;
                let mut state = state_clone.lock().await;

                // 先找到所有需要更新的位置的索引
                let token_info_idx = state
                    .token_manager
                    .tokens
                    .iter()
                    .position(|info| info.token == auth_token_clone);

                let log_idx = state
                    .request_manager
                    .request_logs
                    .iter()
                    .rposition(|log| log.id == log_id);

                // 根据索引更新
                match (token_info_idx, log_idx) {
                    (Some(t_idx), Some(l_idx)) => {
                        state.token_manager.tokens[t_idx].profile = profile.clone();
                        state.request_manager.request_logs[l_idx].token_info.profile = profile;
                    }
                    (Some(t_idx), None) => {
                        state.token_manager.tokens[t_idx].profile = profile;
                    }
                    (None, Some(l_idx)) => {
                        state.request_manager.request_logs[l_idx].token_info.profile = profile;
                    }
                    (None, None) => {}
                }
            });
        }

        state.request_manager.request_logs.push(RequestLog {
            id: next_id,
            timestamp: request_time,
            model: request.model.clone(),
            token_info: TokenInfo {
                token: auth_token.clone(),
                checksum: checksum.clone(),
                profile: None,
                tags: None,
            },
            prompt: None,
            timing: TimingInfo {
                total: 0.0,
                first: None,
            },
            stream: request.stream,
            status: LogStatus::Pending,
            error: None,
        });

        if !*IS_UNLIMITED_REQUEST_LOGS
            && state.request_manager.request_logs.len() > *REQUEST_LOGS_LIMIT
        {
            state.request_manager.request_logs.remove(0);
        }
    }

    // 将消息转换为hex格式
    let hex_data = match super::adapter::encode_chat_message(
        request.messages,
        &model_name,
        current_config.disable_vision(),
        current_config.enable_slow_pool(),
        is_search,
    )
    .await
    {
        Ok(data) => data,
        Err(e) => {
            let mut state = state.lock().await;
            if let Some(log) = state
                .request_manager
                .request_logs
                .iter_mut()
                .rev()
                .find(|log| log.id == current_id)
            {
                log.status = LogStatus::Failed;
                log.error = Some(e.to_string());
            }
            state.request_manager.active_requests -= 1;
            state.request_manager.error_requests += 1;
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    ChatError::RequestFailed("Failed to encode chat message".to_string()).to_json(),
                ),
            ));
        }
    };

    // 构建请求客户端
    let client = build_client(
        &auth_token,
        &checksum,
        if is_search {
            &CURSOR_API2_CHAT_WEB_URL
        } else {
            &CURSOR_API2_CHAT_URL
        },
        true,
    );
    // 添加超时设置
    let response = tokio::time::timeout(
        std::time::Duration::from_secs(*SERVICE_TIMEOUT),
        client.body(hex_data).send(),
    )
    .await;

    // 处理请求结果
    let response = match response {
        Ok(inner_response) => match inner_response {
            Ok(resp) => {
                // 更新请求日志为成功
                {
                    let mut state = state.lock().await;
                    if let Some(log) = state
                        .request_manager
                        .request_logs
                        .iter_mut()
                        .rev()
                        .find(|log| log.id == current_id)
                    {
                        log.status = LogStatus::Success;
                    }
                }
                resp
            }
            Err(e) => {
                // 更新请求日志为失败
                {
                    let mut state = state.lock().await;
                    if let Some(log) = state
                        .request_manager
                        .request_logs
                        .iter_mut()
                        .rev()
                        .find(|log| log.id == current_id)
                    {
                        log.status = LogStatus::Failed;
                        log.error = Some(e.to_string());
                    }
                    state.request_manager.active_requests -= 1;
                    state.request_manager.error_requests += 1;
                }
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ChatError::RequestFailed(e.to_string()).to_json()),
                ));
            }
        },
        Err(_) => {
            // 处理超时错误
            {
                let mut state = state.lock().await;
                if let Some(log) = state
                    .request_manager
                    .request_logs
                    .iter_mut()
                    .rev()
                    .find(|log| log.id == current_id)
                {
                    log.status = LogStatus::Failed;
                    log.error = Some("Request timeout".to_string());
                }
                state.request_manager.active_requests -= 1;
                state.request_manager.error_requests += 1;
            }
            return Err((
                StatusCode::GATEWAY_TIMEOUT,
                Json(ChatError::RequestFailed("Request timeout".to_string()).to_json()),
            ));
        }
    };

    // 释放活动请求计数
    {
        let mut state = state.lock().await;
        state.request_manager.active_requests -= 1;
    }

    let convert_web_ref = current_config.include_web_references();

    if request.stream {
        let response_id = format!("chatcmpl-{}", Uuid::new_v4().simple());
        let is_start = Arc::new(AtomicBool::new(true));
        let start_time = std::time::Instant::now();
        let first_chunk_time = Arc::new(Mutex::new(None::<f64>));
        let decoder = Arc::new(Mutex::new(StreamDecoder::new()));

        // 定义消息处理器的上下文结构体
        struct MessageProcessContext<'a> {
            response_id: &'a str,
            model: &'a str,
            is_start: &'a AtomicBool,
            first_chunk_time: &'a Mutex<Option<f64>>,
            start_time: std::time::Instant,
            state: &'a Mutex<AppState>,
            current_id: u64,
        }

        // 处理消息并生成响应数据的辅助函数
        async fn process_messages(
            messages: Vec<StreamMessage>,
            ctx: &MessageProcessContext<'_>,
        ) -> String {
            let mut response_data = String::new();

            for message in messages {
                match message {
                    StreamMessage::Content(text) => {
                        let is_first = ctx.is_start.load(Ordering::SeqCst);
                        if is_first {
                            if let Ok(mut first_time) = ctx.first_chunk_time.try_lock() {
                                *first_time = Some(ctx.start_time.elapsed().as_secs_f64());
                            }
                        }

                        let response = ChatResponse {
                            id: ctx.response_id.to_string(),
                            object: OBJECT_CHAT_COMPLETION_CHUNK.to_string(),
                            created: chrono::Utc::now().timestamp(),
                            model: if is_first {
                                Some(ctx.model.to_string())
                            } else {
                                None
                            },
                            choices: vec![Choice {
                                index: 0,
                                message: None,
                                delta: Some(Delta {
                                    role: if is_first {
                                        Some(Role::Assistant)
                                    } else {
                                        None
                                    },
                                    content: if is_first {
                                        ctx.is_start.store(false, Ordering::SeqCst);
                                        Some(text.trim_leading_newlines())
                                    } else {
                                        Some(text)
                                    },
                                }),
                                finish_reason: None,
                            }],
                            usage: None,
                        };

                        response_data.push_str(&format!(
                            "data: {}\n\n",
                            serde_json::to_string(&response).unwrap()
                        ));
                    }
                    StreamMessage::StreamEnd => {
                        // 计算总时间和首次片段时间
                        let total_time = ctx.start_time.elapsed().as_secs_f64();
                        let first_time = ctx.first_chunk_time.lock().await.unwrap_or(total_time);

                        {
                            let mut state = ctx.state.lock().await;
                            if let Some(log) = state
                                .request_manager
                                .request_logs
                                .iter_mut()
                                .rev()
                                .find(|log| log.id == ctx.current_id)
                            {
                                log.timing.total = format_time_ms(total_time);
                                log.timing.first = Some(format_time_ms(first_time));
                            }
                        }

                        let response = ChatResponse {
                            id: ctx.response_id.to_string(),
                            object: OBJECT_CHAT_COMPLETION_CHUNK.to_string(),
                            created: chrono::Utc::now().timestamp(),
                            model: None,
                            choices: vec![Choice {
                                index: 0,
                                message: None,
                                delta: Some(Delta {
                                    role: None,
                                    content: None,
                                }),
                                finish_reason: Some(FINISH_REASON_STOP.to_string()),
                            }],
                            usage: None,
                        };
                        response_data.push_str(&format!(
                            "data: {}\n\ndata: [DONE]\n\n",
                            serde_json::to_string(&response).unwrap()
                        ));
                    }
                    StreamMessage::Debug(debug_prompt) => {
                        if let Ok(mut state) = ctx.state.try_lock() {
                            if let Some(log) = state
                                .request_manager
                                .request_logs
                                .iter_mut()
                                .rev()
                                .find(|log| log.id == ctx.current_id)
                            {
                                log.prompt = Some(debug_prompt);
                            }
                        }
                    }
                    _ => {} // 忽略其他消息类型
                }
            }

            response_data
        }

        // 首先处理stream直到获得第一个结果
        let mut stream = response.bytes_stream();
        while !decoder.lock().await.is_first_result_ready() {
            match stream.next().await {
                Some(Ok(chunk)) => {
                    if let Err(StreamError::ChatError(error)) =
                        decoder.lock().await.decode(&chunk, convert_web_ref)
                    {
                        let error_response = error.into_error_response();
                        // 更新请求日志为失败
                        {
                            let mut state = state.lock().await;
                            if let Some(log) = state
                                .request_manager
                                .request_logs
                                .iter_mut()
                                .rev()
                                .find(|log| log.id == current_id)
                            {
                                log.status = LogStatus::Failed;
                                log.error = Some(error_response.native_code());
                                log.timing.total =
                                    format_time_ms(start_time.elapsed().as_secs_f64());
                                state.request_manager.error_requests += 1;
                            }
                        }
                        return Err((
                            error_response.status_code(),
                            Json(error_response.into_common()),
                        ));
                    }
                }
                Some(Err(e)) => {
                    let error_message = format!("Failed to read response chunk: {}", e);
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ChatError::RequestFailed(error_message).to_json()),
                    ));
                }
                None => {
                    // 更新请求日志为失败
                    {
                        let mut state = state.lock().await;
                        if let Some(log) = state
                            .request_manager
                            .request_logs
                            .iter_mut()
                            .rev()
                            .find(|log| log.id == current_id)
                        {
                            log.status = LogStatus::Failed;
                            log.error = Some("Empty stream response".to_string());
                            state.request_manager.error_requests += 1;
                        }
                    }
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(
                            ChatError::RequestFailed("Empty stream response".to_string()).to_json(),
                        ),
                    ));
                }
            }
        }

        // 处理后续的stream
        let stream = stream.then({
            let decoder = decoder.clone();
            let response_id = response_id.clone();
            let model = request.model.clone();
            let is_start = is_start.clone();
            let first_chunk_time = first_chunk_time.clone();
            let state = state.clone();

            move |chunk| {
                let decoder = decoder.clone();
                let response_id = response_id.clone();
                let model = model.clone();
                let is_start = is_start.clone();
                let first_chunk_time = first_chunk_time.clone();
                let state = state.clone();

                async move {
                    let chunk = chunk.unwrap_or_default();

                    let ctx = MessageProcessContext {
                        response_id: &response_id,
                        model: &model,
                        is_start: &is_start,
                        first_chunk_time: &first_chunk_time,
                        start_time,
                        state: &state,
                        current_id,
                    };

                    // 使用decoder处理chunk
                    let messages = match decoder.lock().await.decode(&chunk, convert_web_ref) {
                        Ok(msgs) => msgs,
                        Err(e) => {
                            eprintln!("[警告] Stream error: {}", e);
                            return Ok::<_, Infallible>(Bytes::new());
                        }
                    };

                    let mut response_data = String::new();

                    if let Some(first_msg) = decoder.lock().await.take_first_result() {
                        let first_response = process_messages(first_msg, &ctx).await;
                        response_data.push_str(&first_response);
                    }

                    let current_response = process_messages(messages, &ctx).await;
                    if !current_response.is_empty() {
                        response_data.push_str(&current_response);
                    }

                    Ok(Bytes::from(response_data))
                }
            }
        });

        Ok(Response::builder()
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive")
            .header(CONTENT_TYPE, "text/event-stream")
            .body(Body::from_stream(stream))
            .unwrap())
    } else {
        // 非流式响应
        let start_time = std::time::Instant::now();
        let mut first_chunk_time = None::<f64>;
        let mut decoder = StreamDecoder::new();
        let mut full_text = String::with_capacity(1024);
        let mut stream = response.bytes_stream();

        // 逐个处理chunks
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| {
                let error_message = format!("Failed to read response chunk: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ChatError::RequestFailed(error_message).to_json()),
                )
            })?;

            // 立即处理当前chunk
            match decoder.decode(&chunk, convert_web_ref) {
                Ok(messages) => {
                    for message in messages {
                        match message {
                            StreamMessage::Content(text) => {
                                if first_chunk_time.is_none() {
                                    first_chunk_time = Some(start_time.elapsed().as_secs_f64());
                                }
                                full_text.push_str(&text);
                            }
                            StreamMessage::Debug(debug_prompt) => {
                                if let Ok(mut state) = state.try_lock() {
                                    if let Some(log) = state
                                        .request_manager
                                        .request_logs
                                        .iter_mut()
                                        .rev()
                                        .find(|log| log.id == current_id)
                                    {
                                        log.prompt = Some(debug_prompt);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Err(StreamError::ChatError(error)) => {
                    let error_response = error.into_error_response();
                    return Err((
                        error_response.status_code(),
                        Json(error_response.into_common()),
                    ));
                }
                Err(e) => {
                    let error_response = ErrorResponse {
                        status: ApiStatus::Error,
                        code: Some(500),
                        error: Some(e.to_string()),
                        message: None,
                    };
                    return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)));
                }
            }
        }

        // 检查响应是否为空
        if full_text.is_empty() {
            // 更新请求日志为失败
            {
                let mut state = state.lock().await;
                if let Some(log) = state
                    .request_manager
                    .request_logs
                    .iter_mut()
                    .rev()
                    .find(|log| log.id == current_id)
                {
                    log.status = LogStatus::Failed;
                    log.error = Some("Empty response received".to_string());
                    state.request_manager.error_requests += 1;
                }
            }
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ChatError::RequestFailed("Empty response received".to_string()).to_json()),
            ));
        }

        let response_data = ChatResponse {
            id: format!("chatcmpl-{}", Uuid::new_v4().simple()),
            object: OBJECT_CHAT_COMPLETION.to_string(),
            created: chrono::Utc::now().timestamp(),
            model: Some(request.model),
            choices: vec![Choice {
                index: 0,
                message: Some(Message {
                    role: Role::Assistant,
                    content: MessageContent::Text(full_text.trim_leading_newlines()),
                }),
                delta: None,
                finish_reason: Some(FINISH_REASON_STOP.to_string()),
            }],
            usage: Some(Usage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            }),
        };

        {
            // 更新请求日志时间信息和状态
            let total_time = format_time_ms(start_time.elapsed().as_secs_f64());
            let mut state = state.lock().await;
            if let Some(log) = state
                .request_manager
                .request_logs
                .iter_mut()
                .rev()
                .find(|log| log.id == current_id)
            {
                log.timing.total = total_time;
                log.timing.first = first_chunk_time;
                log.status = LogStatus::Success;
            }
        }

        Ok(Response::builder()
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_string(&response_data).unwrap()))
            .unwrap())
    }
}
