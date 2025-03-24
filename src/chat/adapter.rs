use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use image::guess_format;
use rand::Rng as _;
use reqwest::Client;
use uuid::Uuid;

use crate::{
    app::{
        constant::EMPTY_STRING,
        lazy::DEFAULT_INSTRUCTIONS,
        model::{AppConfig, VisionAbility},
    },
    common::{client::HTTP_CLIENT, utils::encode_message},
};

use super::{
    aiserver::v1::{
        AzureState, ChatExternalLink, ConversationMessage, ExplicitContext, GetChatRequest,
        ImageProto, ModelDetails, WebReference, conversation_message, image_proto,
    },
    constant::{ERR_UNSUPPORTED_GIF, ERR_UNSUPPORTED_IMAGE_FORMAT, LONG_CONTEXT_MODELS},
    model::{Message, MessageContent, Role},
};

fn parse_web_references(text: &str) -> Vec<WebReference> {
    let mut web_refs = Vec::new();
    let lines = text.lines().skip(1); // 跳过 "WebReferences:" 行

    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            break;
        }

        // 跳过序号和空格
        let mut chars = line.chars();
        for c in chars.by_ref() {
            if c == '.' {
                break;
            }
        }
        let remaining = chars.as_str().trim_start();

        // 解析 [title](url) 部分
        if !remaining.starts_with('[') {
            continue;
        }

        let mut title = String::new();
        let mut url = String::new();
        let mut chunk = String::new();
        let mut current = &mut title;
        let mut state = 0; // 0: title, 1: url, 2: chunk

        let mut chars = remaining.chars();
        chars.next(); // 跳过 '['

        while let Some(c) = chars.next() {
            match (state, c) {
                (0, ']') => {
                    state = 1;
                    if chars.next() != Some('(') {
                        break;
                    }
                    current = &mut url;
                }
                (1, ')') => {
                    state = 2;
                    if chars.next() == Some('<') {
                        current = &mut chunk;
                    } else {
                        break;
                    }
                }
                (2, '>') => break,
                (_, c) => current.push(c),
            }
        }

        web_refs.push(WebReference { title, url, chunk });
    }

    web_refs
}

async fn process_chat_inputs(
    inputs: Vec<Message>,
    disable_vision: bool,
) -> (String, Vec<ConversationMessage>, Vec<String>) {
    // 收集 system 指令
    let instructions = inputs
        .iter()
        .filter(|input| input.role == Role::System)
        .map(|input| match &input.content {
            MessageContent::Text(text) => text.clone(),
            MessageContent::Vision(contents) => contents
                .iter()
                .filter_map(|content| {
                    if content.content_type == "text" {
                        content.text.clone()
                    } else {
                        None
                    }
                })
                .collect::<Vec<String>>()
                .join("\n"),
        })
        .collect::<Vec<String>>()
        .join("\n\n");

    // 使用默认指令或收集到的指令
    let instructions = if instructions.is_empty() {
        DEFAULT_INSTRUCTIONS.clone()
    } else {
        instructions
    };

    // 过滤出 user 和 assistant 对话
    let mut chat_inputs: Vec<Message> = inputs
        .into_iter()
        .filter(|input| input.role == Role::User || input.role == Role::Assistant)
        .collect();

    // 处理空对话情况
    if chat_inputs.is_empty() {
        return (
            instructions,
            vec![ConversationMessage {
                text: EMPTY_STRING.into(),
                r#type: conversation_message::MessageType::Human as i32,
                attached_code_chunks: vec![],
                codebase_context_chunks: vec![],
                commits: vec![],
                pull_requests: vec![],
                git_diffs: vec![],
                assistant_suggested_diffs: vec![],
                interpreter_results: vec![],
                images: vec![],
                attached_folders: vec![],
                approximate_lint_errors: vec![],
                bubble_id: Uuid::new_v4().to_string(),
                server_bubble_id: None,
                attached_folders_new: vec![],
                lints: vec![],
                user_responses_to_suggested_code_blocks: vec![],
                relevant_files: vec![],
                tool_results: vec![],
                notepads: vec![],
                is_capability_iteration: Some(false),
                capabilities: vec![],
                edit_trail_contexts: vec![],
                suggested_code_blocks: vec![],
                diffs_for_compressing_files: vec![],
                multi_file_linter_errors: vec![],
                diff_histories: vec![],
                recently_viewed_files: vec![],
                recent_locations_history: vec![],
                is_agentic: false,
                file_diff_trajectories: vec![],
                conversation_summary: None,
                existed_subsequent_terminal_command: false,
                existed_previous_terminal_command: false,
                docs_references: vec![],
                web_references: vec![],
                git_context: None,
                attached_folders_list_dir_results: vec![],
                cached_conversation_summary: None,
                human_changes: vec![],
                attached_human_changes: false,
                summarized_composers: vec![],
                cursor_rules: vec![],
            }],
            vec![],
        );
    }

    // 处理 WebReferences 开头的 assistant 消息
    chat_inputs = chat_inputs
        .into_iter()
        .map(|mut input| {
            if let (Role::Assistant, MessageContent::Text(text)) = (&input.role, &input.content) {
                if text.starts_with("WebReferences:") {
                    if let Some(pos) = text.find("\n\n") {
                        input.content = MessageContent::Text(text[pos + 2..].to_owned());
                    }
                }
            }
            input
        })
        .collect();

    // 如果第一条是 assistant，插入空的 user 消息
    if chat_inputs
        .first()
        .is_some_and(|input| input.role == Role::Assistant)
    {
        chat_inputs.insert(
            0,
            Message {
                role: Role::User,
                content: MessageContent::Text(EMPTY_STRING.into()),
            },
        );
    }

    // 处理连续相同角色的情况
    let mut i = 1;
    while i < chat_inputs.len() {
        if chat_inputs[i].role == chat_inputs[i - 1].role {
            let insert_role = if chat_inputs[i].role == Role::User {
                Role::Assistant
            } else {
                Role::User
            };
            chat_inputs.insert(
                i,
                Message {
                    role: insert_role,
                    content: MessageContent::Text(EMPTY_STRING.into()),
                },
            );
        }
        i += 1;
    }

    // 确保最后一条是 user
    if chat_inputs
        .last()
        .is_some_and(|input| input.role == Role::Assistant)
    {
        chat_inputs.push(Message {
            role: Role::User,
            content: MessageContent::Text(EMPTY_STRING.into()),
        });
    }

    // 转换为 proto messages
    let mut messages = Vec::new();
    for input in chat_inputs {
        let (text, images) = match input.content {
            MessageContent::Text(text) => (text, vec![]),
            MessageContent::Vision(contents) => {
                let mut text_parts = Vec::new();
                let mut images = Vec::new();

                for content in contents {
                    match content.content_type.as_str() {
                        "text" => {
                            if let Some(text) = content.text {
                                text_parts.push(text);
                            }
                        }
                        "image_url" => {
                            if !disable_vision {
                                if let Some(image_url) = &content.image_url {
                                    let url = image_url.url.clone();
                                    let client = HTTP_CLIENT.read().clone();
                                    let result = tokio::spawn(async move {
                                        fetch_image_data(&url, client).await
                                    });
                                    if let Ok(Ok((image_data, dimensions))) = result.await {
                                        images.push(ImageProto {
                                            data: image_data,
                                            dimension: dimensions,
                                        });
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                (text_parts.join("\n"), images)
            }
        };

        let (text, web_references) =
            if input.role == Role::Assistant && text.starts_with("WebReferences:") {
                if let Some(pos) = text.find("\n\n") {
                    let (web_refs_text, content_text) = text.split_at(pos);
                    (
                        content_text[2..].to_string(), // 跳过 "\n\n"
                        parse_web_references(web_refs_text),
                    )
                } else {
                    (text, vec![])
                }
            } else {
                (text, vec![])
            };

        messages.push(ConversationMessage {
            text,
            r#type: if input.role == Role::User {
                conversation_message::MessageType::Human as i32
            } else {
                conversation_message::MessageType::Ai as i32
            },
            attached_code_chunks: vec![],
            codebase_context_chunks: vec![],
            commits: vec![],
            pull_requests: vec![],
            git_diffs: vec![],
            assistant_suggested_diffs: vec![],
            interpreter_results: vec![],
            images,
            attached_folders: vec![],
            approximate_lint_errors: vec![],
            bubble_id: Uuid::new_v4().to_string(),
            server_bubble_id: None,
            attached_folders_new: vec![],
            lints: vec![],
            user_responses_to_suggested_code_blocks: vec![],
            relevant_files: vec![],
            tool_results: vec![],
            notepads: vec![],
            is_capability_iteration: None,
            capabilities: vec![],
            edit_trail_contexts: vec![],
            suggested_code_blocks: vec![],
            diffs_for_compressing_files: vec![],
            multi_file_linter_errors: vec![],
            diff_histories: vec![],
            recently_viewed_files: vec![],
            recent_locations_history: vec![],
            is_agentic: false,
            file_diff_trajectories: vec![],
            conversation_summary: None,
            existed_subsequent_terminal_command: false,
            existed_previous_terminal_command: false,
            docs_references: vec![],
            web_references,
            git_context: None,
            attached_folders_list_dir_results: vec![],
            cached_conversation_summary: None,
            human_changes: vec![],
            attached_human_changes: false,
            summarized_composers: vec![],
            cursor_rules: vec![],
        });
    }

    let mut urls = Vec::new();
    if let Some(last_msg) = messages.last() {
        if last_msg.r#type == conversation_message::MessageType::Human as i32 {
            let text = &last_msg.text;
            let mut chars = text.chars().peekable();

            while let Some(c) = chars.next() {
                if c == '@' {
                    let mut url = String::new();
                    while let Some(&next_char) = chars.peek() {
                        if next_char.is_whitespace() {
                            break;
                        }
                        url.push(chars.next().unwrap());
                    }
                    if let Ok(parsed_url) = url::Url::parse(&url) {
                        if parsed_url.scheme() == "http" || parsed_url.scheme() == "https" {
                            urls.push(url);
                        }
                    }
                }
            }
        }
    }

    (instructions, messages, urls)
}

async fn fetch_image_data(
    url: &str,
    client: Client,
) -> Result<(Vec<u8>, Option<image_proto::Dimension>), Box<dyn std::error::Error + Send + Sync>> {
    // 在进入异步操作前获取并释放锁
    let vision_ability = AppConfig::get_vision_ability();

    match vision_ability {
        VisionAbility::None => Err("图片功能已禁用".into()),

        VisionAbility::Base64 => {
            if !url.starts_with("data:image/") {
                return Err("仅支持 base64 编码的图片".into());
            }
            process_base64_image(url)
        }

        VisionAbility::All => {
            if url.starts_with("data:image/") {
                process_base64_image(url)
            } else {
                process_http_image(url, client).await
            }
        }
    }
}

// 处理 base64 编码的图片
fn process_base64_image(
    url: &str,
) -> Result<(Vec<u8>, Option<image_proto::Dimension>), Box<dyn std::error::Error + Send + Sync>> {
    let parts: Vec<&str> = url.split("base64,").collect();
    if parts.len() != 2 {
        return Err("无效的 base64 图片格式".into());
    }

    // 检查图片格式
    let format = parts[0].to_lowercase();
    if !format.contains("png")
        && !format.contains("jpeg")
        && !format.contains("jpg")
        && !format.contains("webp")
        && !format.contains("gif")
    {
        return Err(ERR_UNSUPPORTED_IMAGE_FORMAT.into());
    }

    let image_data = BASE64.decode(parts[1])?;

    // 检查是否为动态 GIF
    if format.contains("gif") {
        if let Ok(frames) = gif::DecodeOptions::new().read_info(std::io::Cursor::new(&image_data)) {
            if frames.into_iter().count() > 1 {
                return Err(ERR_UNSUPPORTED_GIF.into());
            }
        }
    }

    // 获取图片尺寸
    let dimensions = if let Ok(img) = image::load_from_memory(&image_data) {
        Some(image_proto::Dimension {
            width: img.width() as i32,
            height: img.height() as i32,
        })
    } else {
        None
    };

    Ok((image_data, dimensions))
}

// 处理 HTTP 图片 URL
async fn process_http_image(
    url: &str,
    client: Client,
) -> Result<(Vec<u8>, Option<image_proto::Dimension>), Box<dyn std::error::Error + Send + Sync>> {
    let response = client.get(url).send().await?;
    let image_data = response.bytes().await?.to_vec();
    let format = guess_format(&image_data)?;

    // 检查图片格式
    match format {
        image::ImageFormat::Png | image::ImageFormat::Jpeg | image::ImageFormat::WebP => {
            // 这些格式都支持
        }
        image::ImageFormat::Gif => {
            if let Ok(frames) =
                gif::DecodeOptions::new().read_info(std::io::Cursor::new(&image_data))
            {
                if frames.into_iter().count() > 1 {
                    return Err(ERR_UNSUPPORTED_GIF.into());
                }
            }
        }
        _ => return Err(ERR_UNSUPPORTED_IMAGE_FORMAT.into()),
    }

    // 获取图片尺寸
    let dimensions = if let Ok(img) = image::load_from_memory_with_format(&image_data, format) {
        Some(image_proto::Dimension {
            width: img.width() as i32,
            height: img.height() as i32,
        })
    } else {
        None
    };

    Ok((image_data, dimensions))
}

pub async fn encode_chat_message(
    inputs: Vec<Message>,
    model_name: &str,
    disable_vision: bool,
    enable_slow_pool: bool,
    is_search: bool,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    // 在进入异步操作前获取并释放锁
    let enable_slow_pool = { if enable_slow_pool { Some(true) } else { None } };

    let (instructions, messages, urls) = process_chat_inputs(inputs, disable_vision).await;

    let explicit_context = if !instructions.trim().is_empty() {
        Some(ExplicitContext {
            context: instructions,
            repo_context: None,
            rules: vec![],
        })
    } else {
        None
    };

    let base_uuid = rand::rng().random::<u16>();
    let external_links = urls
        .into_iter()
        .enumerate()
        .map(|(i, url)| {
            let uuid = base_uuid.wrapping_add(i as u16);
            ChatExternalLink {
                url,
                uuid: uuid.to_string(),
            }
        })
        .collect();

    let chat = GetChatRequest {
        current_file: None,
        conversation: messages,
        repositories: vec![],
        explicit_context,
        workspace_root_path: None,
        code_blocks: vec![],
        model_details: Some(ModelDetails {
            model_name: Some(model_name.to_string()),
            api_key: None,
            enable_ghost_mode: None,
            azure_state: Some(AzureState {
                api_key: String::new(),
                base_url: String::new(),
                deployment: String::new(),
                use_azure: false,
            }),
            enable_slow_pool,
            openai_api_base_url: None,
        }),
        documentation_identifiers: vec![],
        request_id: Uuid::new_v4().to_string(),
        linter_errors: None,
        summary: None,
        summary_up_until_index: None,
        allow_long_file_scan: Some(false),
        is_bash: Some(false),
        conversation_id: Uuid::new_v4().to_string(),
        can_handle_filenames_after_language_ids: Some(true),
        use_web: if is_search {
            Some("full_search".to_string())
        } else {
            None
        },
        quotes: vec![],
        debug_info: None,
        workspace_id: None,
        external_links,
        commit_notes: vec![],
        long_context_mode: Some(LONG_CONTEXT_MODELS.contains(&model_name)),
        is_eval: Some(false),
        desired_max_tokens: None,
        context_ast: None,
        is_composer: None,
        runnable_code_blocks: Some(false),
        should_cache: Some(false),
        allow_model_fallbacks: None,
        number_of_times_shown_fallback_model_warning: None,
    };

    encode_message(&chat, true)
}
