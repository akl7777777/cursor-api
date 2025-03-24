mod checksum;
use ::base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
pub use checksum::*;
mod token;
use prost::Message as _;
pub use token::*;
mod base64;
pub use base64::*;

use super::model::{
    token::TokenPayload,
    userinfo::{StripeProfile, TokenProfile, UsageProfile, UserProfile},
};
use crate::{
    app::{
        constant::{COMMA, FALSE, TRUE},
        lazy::{CURSOR_API2_CHAT_MODELS_URL, TOKEN_DELIMITER, USE_COMMA_DELIMITER},
    },
    chat::{
        aiserver::v1::{AvailableModelsRequest, AvailableModelsResponse},
        constant::{
            ANTHROPIC, CREATED, CURSOR, DEEPSEEK, GOOGLE, MODEL_OBJECT, OPENAI, UNKNOWN, XAI,
        },
        model::Model,
    },
};

pub fn parse_bool_from_env(key: &str, default: bool) -> bool {
    std::env::var(key)
        .ok()
        .map(|v| match v.to_lowercase().as_str() {
            TRUE | "1" => true,
            FALSE | "0" => false,
            _ => default,
        })
        .unwrap_or(default)
}

pub fn parse_string_from_env(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

pub fn parse_ascii_char_from_env(key: &str, default: char) -> char {
    std::env::var(key)
        .ok()
        .and_then(|v| {
            let chars: Vec<char> = v.chars().collect();
            if chars.len() == 1 && chars[0].is_ascii() {
                Some(chars[0])
            } else {
                None
            }
        })
        .unwrap_or(default)
}

pub fn parse_usize_from_env(key: &str, default: usize) -> usize {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

pub trait TrimNewlines {
    fn trim_leading_newlines(self) -> Self;
}

impl TrimNewlines for String {
    #[inline(always)]
    fn trim_leading_newlines(mut self) -> Self {
        let bytes = self.as_bytes();
        if bytes.len() >= 2 && bytes[0] == b'\n' && bytes[1] == b'\n' {
            unsafe {
                let start_ptr = self.as_mut_ptr();
                let new_len = self.len() - 2;
                std::ptr::copy(start_ptr.add(2), start_ptr, new_len);
                self.as_mut_vec().set_len(new_len);
            }
        }
        self
    }
}

pub async fn get_token_profile(auth_token: &str) -> Option<TokenProfile> {
    let user_id = extract_user_id(auth_token)?;

    // 构建请求客户端
    let client = super::client::build_usage_client(&user_id, auth_token);

    // 发送请求并获取响应
    // let response = client.send().await.ok()?;
    // let bytes = response.bytes().await?;
    // println!("Raw response bytes: {:?}", bytes);
    // let usage = serde_json::from_str::<UsageProfile>(&text).ok()?;
    let usage = client
        .send()
        .await
        .ok()?
        .json::<UsageProfile>()
        .await
        .ok()?;

    let user = get_user_profile(auth_token).await?;

    // 从 Stripe 获取用户资料
    let stripe = get_stripe_profile(auth_token).await?;

    // 映射响应数据到 TokenProfile
    Some(TokenProfile {
        usage,
        user,
        stripe,
    })
}

pub async fn get_stripe_profile(auth_token: &str) -> Option<StripeProfile> {
    let client = super::client::build_profile_client(auth_token);
    let response = client
        .send()
        .await
        .ok()?
        .json::<StripeProfile>()
        .await
        .ok()?;
    Some(response)
}

pub async fn get_user_profile(auth_token: &str) -> Option<UserProfile> {
    let user_id = extract_user_id(auth_token)?;

    // 构建请求客户端
    let client = super::client::build_userinfo_client(&user_id, auth_token);

    // 发送请求并获取响应
    let user_profile = client.send().await.ok()?.json::<UserProfile>().await.ok()?;

    Some(user_profile)
}

pub async fn get_available_models(auth_token: &str, checksum: &str) -> Option<Vec<Model>> {
    let client =
        super::client::build_client(auth_token, checksum, &CURSOR_API2_CHAT_MODELS_URL, false);
    let request = AvailableModelsRequest {
        is_nightly: true,
        include_long_context_models: true,
    };
    let response = client
        .body(encode_message(&request, false).unwrap())
        .send()
        .await
        .ok()?
        .bytes()
        .await
        .ok()?;
    let available_models = AvailableModelsResponse::decode(response.as_ref()).ok()?;
    Some(
        available_models
            .models
            .into_iter()
            .map(|model| Model {
                id: model.name.clone(),
                created: CREATED,
                object: MODEL_OBJECT,
                owned_by: {
                    let mut chars = model.name.chars();
                    match chars.next() {
                        Some('g') => match chars.next() {
                            Some('p') => OPENAI, // g + p → "gp" (gpt)
                            Some('e') => GOOGLE, // g + e → "ge" (gemini)
                            Some('r') => XAI,    // g + r → "ge" (grok)
                            _ => UNKNOWN,
                        },
                        Some('o') => match chars.next() {
                            // o 开头需要二次判断
                            Some('1') | Some('3') => OPENAI, // o1/o3 系列
                            _ => UNKNOWN,
                        },
                        Some('c') => match chars.next() {
                            Some('l') => ANTHROPIC, // c + l → "cl" (claude)
                            Some('u') => CURSOR,    // c + u → "cu" (cursor)
                            _ => UNKNOWN,
                        },
                        Some('d') if chars.next() == Some('e') => DEEPSEEK, // d + e → "de" (deepseek)
                        // 其他情况
                        _ => UNKNOWN,
                    }
                },
            })
            .collect(),
    )
}

pub fn validate_token_and_checksum(auth_token: &str) -> Option<(String, String)> {
    // 尝试使用自定义分隔符查找
    let mut delimiter_pos = auth_token.rfind(*TOKEN_DELIMITER);

    // 如果自定义分隔符未找到，并且 USE_COMMA_DELIMITER 为 true，则尝试使用逗号
    if delimiter_pos.is_none() && *USE_COMMA_DELIMITER {
        delimiter_pos = auth_token.rfind(COMMA);
    }

    // 如果最终都没有找到分隔符，则返回 None
    let comma_pos = delimiter_pos?;

    // 使用找到的分隔符位置分割字符串
    let (token_part, checksum) = auth_token.split_at(comma_pos);
    let checksum = &checksum[1..]; // 跳过逗号

    // 解析 token - 为了向前兼容,忽略最后一个:或%3A前的内容
    let colon_pos = token_part.rfind(':');
    let encoded_colon_pos = token_part.rfind("%3A");

    let token = match (colon_pos, encoded_colon_pos) {
        (None, None) => token_part, // 最简单的构成: token,checksum
        (Some(pos1), None) => &token_part[(pos1 + 1)..],
        (None, Some(pos2)) => &token_part[(pos2 + 3)..],
        (Some(pos1), Some(pos2)) => {
            let pos = pos1.max(pos2);
            let start = if pos == pos2 { pos + 3 } else { pos + 1 };
            &token_part[start..]
        }
    };

    // 验证 token 和 checksum 有效性
    if validate_token(token) && validate_checksum(checksum) {
        Some((token.to_string(), generate_checksum_with_repair(checksum)))
    } else {
        None
    }
}

pub fn extract_token(auth_token: &str) -> Option<String> {
    // 尝试使用自定义分隔符查找
    let mut delimiter_pos = auth_token.rfind(*TOKEN_DELIMITER);

    // 如果自定义分隔符未找到，并且 USE_COMMA_DELIMITER 为 true，则尝试使用逗号
    if delimiter_pos.is_none() && *USE_COMMA_DELIMITER {
        delimiter_pos = auth_token.rfind(COMMA);
    }

    // 根据是否找到分隔符来确定 token_part
    let token_part = match delimiter_pos {
        Some(pos) => &auth_token[..pos],
        None => auth_token,
    };

    // 向前兼容
    let colon_pos = token_part.rfind(':');
    let encoded_colon_pos = token_part.rfind("%3A");

    let token = match (colon_pos, encoded_colon_pos) {
        (None, None) => token_part,
        (Some(pos1), None) => &token_part[(pos1 + 1)..],
        (None, Some(pos2)) => &token_part[(pos2 + 3)..],
        (Some(pos1), Some(pos2)) => {
            let pos = pos1.max(pos2);
            let start = if pos == pos2 { pos + 3 } else { pos + 1 };
            &token_part[start..]
        }
    };

    // 验证 token 有效性
    if validate_token(token) {
        Some(token.to_string())
    } else {
        None
    }
}

pub fn format_time_ms(seconds: f64) -> f64 {
    (seconds * 1000.0).round() / 1000.0
}

use crate::chat::config::key_config;

/// 将 JWT token 转换为 TokenInfo
pub fn token_to_tokeninfo(auth_token: &str) -> Option<key_config::TokenInfo> {
    let (token, checksum) = validate_token_and_checksum(auth_token)?;

    // JWT token 由3部分组成，用 . 分隔
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return None;
    }

    // 解码 payload (第二部分)
    let payload = match URL_SAFE_NO_PAD.decode(parts[1]) {
        Ok(decoded) => decoded,
        Err(_) => return None,
    };

    // 将 payload 转换为字符串
    let payload_str = match String::from_utf8(payload) {
        Ok(s) => s,
        Err(_) => return None,
    };

    // 解析为 TokenPayload
    let payload: TokenPayload = match serde_json::from_str(&payload_str) {
        Ok(p) => p,
        Err(_) => return None,
    };

    let (machine_id_hash, mac_id_hash) = extract_hashes(&checksum)?;

    // 构建 TokenInfo
    Some(key_config::TokenInfo {
        sub: payload.sub,
        exp: payload.exp,
        randomness: payload.randomness,
        signature: parts[2].to_string(),
        machine_id: machine_id_hash,
        mac_id: mac_id_hash,
    })
}

/// 将 TokenInfo 转换为 JWT token
pub fn tokeninfo_to_token(info: &key_config::TokenInfo) -> Option<(String, String)> {
    // 构建 payload
    let payload = TokenPayload {
        sub: info.sub.clone(),
        exp: info.exp,
        randomness: info.randomness.clone(),
        time: (info.exp - 2592000000).to_string(), // exp - 30000天
        iss: ISSUER.to_string(),
        scope: SCOPE.to_string(),
        aud: AUDIENCE.to_string(),
    };

    let payload_str = match serde_json::to_string(&payload) {
        Ok(s) => s,
        Err(_) => return None,
    };

    let payload_b64 = URL_SAFE_NO_PAD.encode(payload_str.as_bytes());

    // 从 TokenInfo 中获取 machine_id 和 mac_id 的 hex 字符串
    let device_id = hex::encode(&info.machine_id);
    let mac_addr = if !info.mac_id.is_empty() {
        Some(hex::encode(&info.mac_id))
    } else {
        None
    };

    // 组合 token
    Some((
        format!("{}.{}.{}", HEADER_B64, payload_b64, info.signature),
        generate_checksum(&device_id, mac_addr.as_deref()),
    ))
}

pub fn encode_message(
    message: &impl prost::Message,
    with_gzip: bool,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let mut encoded = Vec::new();
    message.encode(&mut encoded)?;

    if !with_gzip {
        return Ok(encoded);
    }
    // 构造 5 字节头部 [0x00, len_be_bytes...]
    let mut header = Vec::with_capacity(5);
    header.push(0x00); // 压缩标记位

    // 将长度转换为 u32 大端字节（显式长度检查）
    let len = u32::try_from(encoded.len()).map_err(|_| "Message length exceeds u32::MAX")?; // 明确错误类型
    header.extend_from_slice(&len.to_be_bytes());

    // 组合最终数据
    let mut result = header;
    result.extend(encoded);

    Ok(result)
}
