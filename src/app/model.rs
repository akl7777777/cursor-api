use crate::{
    chat::model::Message,
    common::{
        model::{ApiStatus, userinfo::TokenProfile},
        utils::{generate_checksum_with_repair, get_token_profile},
    },
};
use memmap2::{MmapMut, MmapOptions};
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs::OpenOptions};

mod usage_check;
pub use usage_check::UsageCheck;
mod vision_ability;
pub use vision_ability::VisionAbility;
mod config;
pub use config::AppConfig;
mod proxies;
pub use proxies::Proxies;
mod build_key;
pub use build_key::*;

use super::{
    constant::{STATUS_FAILED, STATUS_PENDING, STATUS_SUCCESS},
    lazy::{LOGS_FILE_PATH, TOKENS_FILE_PATH},
};

// 页面内容类型枚举
#[derive(Clone, Serialize, Deserialize, Archive, RkyvDeserialize, RkyvSerialize)]
#[serde(tag = "type", content = "content")]
pub enum PageContent {
    #[serde(rename = "default")]
    Default, // 默认行为
    #[serde(rename = "text")]
    Text(String), // 纯文本
    #[serde(rename = "html")]
    Html(String), // HTML 内容
}

impl Default for PageContent {
    fn default() -> Self {
        Self::Default
    }
}

#[derive(Clone, Default, Archive, RkyvDeserialize, RkyvSerialize)]
pub struct Pages {
    pub root_content: PageContent,
    pub logs_content: PageContent,
    pub config_content: PageContent,
    pub tokeninfo_content: PageContent,
    pub shared_styles_content: PageContent,
    pub shared_js_content: PageContent,
    pub about_content: PageContent,
    pub readme_content: PageContent,
    pub api_content: PageContent,
    pub build_key_content: PageContent,
}

#[derive(Serialize, Clone, Archive, RkyvDeserialize, RkyvSerialize)]
pub struct TokenGroup {
    pub index: u16,
    pub name: String,
    pub tokens: Vec<TokenInfo>,
    #[serde(default)]
    pub enabled: bool,
}

// Token管理器
#[derive(Clone, Archive, RkyvDeserialize, RkyvSerialize)]
pub struct TokenManager {
    pub tokens: Vec<TokenInfo>,
    pub tags: HashSet<String>, // 存储所有已使用的标签
}

// 请求统计管理器
#[derive(Clone, Archive, RkyvDeserialize, RkyvSerialize)]
pub struct RequestStatsManager {
    pub total_requests: u64,
    pub active_requests: u64,
    pub error_requests: u64,
    pub request_logs: Vec<RequestLog>,
}

#[derive(Clone, Archive, RkyvDeserialize, RkyvSerialize)]
pub struct AppState {
    pub token_manager: TokenManager,
    pub request_manager: RequestStatsManager,
}

impl TokenManager {
    pub fn new(tokens: Vec<TokenInfo>) -> Self {
        let mut tags = HashSet::new();
        for token in &tokens {
            if let Some(token_tags) = &token.tags {
                tags.extend(token_tags.iter().cloned());
            }
        }

        Self { tokens, tags }
    }

    pub fn update_global_tags(&mut self, new_tags: &[String]) {
        // 将新标签添加到全局标签集合中
        self.tags.extend(new_tags.iter().cloned());
    }

    pub fn update_tokens_tags(
        &mut self,
        tokens: Vec<String>,
        new_tags: Vec<String>,
    ) -> Result<(), &'static str> {
        // 创建tokens的HashSet用于快速查找
        let tokens_set: HashSet<_> = tokens.iter().collect();

        // 更新指定tokens的标签
        for token_info in &mut self.tokens {
            if tokens_set.contains(&token_info.token) {
                token_info.tags = Some(new_tags.clone());
            }
        }

        // 更新全局标签集合
        self.tags = self
            .tokens
            .iter()
            .filter_map(|t| t.tags.clone())
            .flatten()
            .collect();

        Ok(())
    }

    pub fn get_tokens_by_tag(&self, tag: &str) -> Vec<&TokenInfo> {
        self.tokens
            .iter()
            .filter(|t| {
                t.tags
                    .as_ref()
                    .is_some_and(|tags| tags.contains(&tag.to_string()))
            })
            .collect()
    }

    pub fn update_checksum(&mut self) {
        for token_info in self.tokens.iter_mut() {
            token_info.checksum = generate_checksum_with_repair(&token_info.checksum);
        }
    }

    pub async fn save_tokens(&self) -> Result<(), Box<dyn std::error::Error>> {
        let bytes = rkyv::to_bytes::<_, 256>(self)?;

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&*TOKENS_FILE_PATH)?;

        if bytes.len() > usize::MAX / 2 {
            return Err("Token数据过大".into());
        }

        file.set_len(bytes.len() as u64)?;
        let mut mmap = unsafe { MmapMut::map_mut(&file)? };
        mmap.copy_from_slice(&bytes);
        mmap.flush()?;

        Ok(())
    }

    pub async fn load_tokens() -> Result<Self, Box<dyn std::error::Error>> {
        let file = match OpenOptions::new().read(true).open(&*TOKENS_FILE_PATH) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::new(Vec::new()));
            }
            Err(e) => return Err(Box::new(e)),
        };

        if file.metadata()?.len() > usize::MAX as u64 {
            return Err("Token文件过大".into());
        }

        let mmap = unsafe { MmapOptions::new().map(&file)? };
        let archived = unsafe { rkyv::archived_root::<Self>(&mmap) };
        Ok(archived.deserialize(&mut rkyv::Infallible)?)
    }
}

impl RequestStatsManager {
    pub fn new(request_logs: Vec<RequestLog>) -> Self {
        Self {
            total_requests: request_logs.len() as u64,
            active_requests: 0,
            error_requests: request_logs
                .iter()
                .filter(|log| matches!(log.status, LogStatus::Failed))
                .count() as u64,
            request_logs,
        }
    }

    pub async fn save_logs(&self) -> Result<(), Box<dyn std::error::Error>> {
        let bytes = rkyv::to_bytes::<_, 256>(&self.request_logs)?;

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&*LOGS_FILE_PATH)?;

        if bytes.len() > usize::MAX / 2 {
            return Err("日志数据过大".into());
        }

        file.set_len(bytes.len() as u64)?;
        let mut mmap = unsafe { MmapMut::map_mut(&file)? };
        mmap.copy_from_slice(&bytes);
        mmap.flush()?;

        Ok(())
    }

    pub async fn load_logs() -> Result<Vec<RequestLog>, Box<dyn std::error::Error>> {
        let file = match OpenOptions::new().read(true).open(&*LOGS_FILE_PATH) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Vec::new());
            }
            Err(e) => return Err(Box::new(e)),
        };

        if file.metadata()?.len() > usize::MAX as u64 {
            return Err("日志文件过大".into());
        }

        let mmap = unsafe { MmapOptions::new().map(&file)? };
        let archived = unsafe { rkyv::archived_root::<Vec<RequestLog>>(&mmap) };
        Ok(archived.deserialize(&mut rkyv::Infallible)?)
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppState {
    pub fn new() -> Self {
        // 尝试加载保存的数据
        let (request_logs, mut token_manager) = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let logs = RequestStatsManager::load_logs().await.unwrap_or_default();
                let token_manager = TokenManager::load_tokens()
                    .await
                    .unwrap_or_else(|_| TokenManager::new(Vec::new()));
                (logs, token_manager)
            })
        });

        // 查询缺失的 token profiles
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                for token_info in token_manager.tokens.iter_mut() {
                    if token_info.profile.is_none() {
                        token_info.profile = get_token_profile(&token_info.token).await;
                    }
                }
            })
        });

        Self {
            token_manager,
            request_manager: RequestStatsManager::new(request_logs),
        }
    }

    pub async fn save_state(&self) -> Result<(), Box<dyn std::error::Error>> {
        // 并行保存 logs 和 tokens
        let (logs_result, tokens_result) = tokio::join!(
            self.request_manager.save_logs(),
            self.token_manager.save_tokens()
        );

        logs_result?;
        tokens_result?;
        Ok(())
    }
}

#[derive(Clone, Archive, RkyvDeserialize, RkyvSerialize)]
pub enum LogStatus {
    Pending,
    Success,
    Failed,
}

impl Serialize for LogStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str_name())
    }
}

impl LogStatus {
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::Pending => STATUS_PENDING,
            Self::Success => STATUS_SUCCESS,
            Self::Failed => STATUS_FAILED,
        }
    }

    pub fn from_str_name(s: &str) -> Option<Self> {
        match s {
            STATUS_PENDING => Some(Self::Pending),
            STATUS_SUCCESS => Some(Self::Success),
            STATUS_FAILED => Some(Self::Failed),
            _ => None,
        }
    }
}

// 请求日志
#[derive(Serialize, Clone, Archive, RkyvDeserialize, RkyvSerialize)]
pub struct RequestLog {
    pub id: u64,
    pub timestamp: chrono::DateTime<chrono::Local>,
    pub model: String,
    pub token_info: TokenInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    pub timing: TimingInfo,
    pub stream: bool,
    pub status: LogStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize, Clone, Archive, RkyvDeserialize, RkyvSerialize)]
pub struct TimingInfo {
    pub total: f64, // 总用时(秒)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first: Option<f64>, // 首字时间(秒)
}

// 聊天请求
#[derive(Deserialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub stream: bool,
}

// 用于存储 token 信息
#[derive(Clone, Serialize, Archive, RkyvSerialize, RkyvDeserialize)]
pub struct TokenInfo {
    pub token: String,
    pub checksum: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<TokenProfile>,
    pub tags: Option<Vec<String>>,
}

// TokenUpdateRequest 结构体
#[derive(Deserialize)]
pub struct TokenUpdateRequest {
    pub tokens: String,
}

#[derive(Deserialize)]
pub struct TokenAddRequest {
    pub tokens: Vec<TokenAddRequestTokenInfo>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct TokenAddRequestTokenInfo {
    pub token: String,
    #[serde(default)]
    pub checksum: Option<String>,
}

// TokensDeleteRequest 结构体
#[derive(Deserialize)]
pub struct TokensDeleteRequest {
    #[serde(default)]
    pub tokens: Vec<String>,
    #[serde(default)]
    pub expectation: TokensDeleteResponseExpectation,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TokensDeleteResponseExpectation {
    #[default]
    Simple,
    UpdatedTokens,
    FailedTokens,
    Detailed,
}

impl TokensDeleteResponseExpectation {
    pub fn needs_updated_tokens(&self) -> bool {
        matches!(
            self,
            TokensDeleteResponseExpectation::UpdatedTokens
                | TokensDeleteResponseExpectation::Detailed
        )
    }

    pub fn needs_failed_tokens(&self) -> bool {
        matches!(
            self,
            TokensDeleteResponseExpectation::FailedTokens
                | TokensDeleteResponseExpectation::Detailed
        )
    }
}

// TokensDeleteResponse 结构体
#[derive(Serialize)]
pub struct TokensDeleteResponse {
    pub status: ApiStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_tokens: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_tokens: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct TokenInfoResponse {
    pub status: ApiStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<Vec<TokenInfo>>,
    pub tokens_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

// 标签相关的请求/响应结构体
#[derive(Deserialize)]
pub struct TokenTagsUpdateRequest {
    pub tokens: Vec<String>,
    pub tags: Vec<String>,
}

#[derive(Serialize)]
pub struct TokenTagsResponse {
    pub status: ApiStatus,
    pub message: Option<String>,
}
