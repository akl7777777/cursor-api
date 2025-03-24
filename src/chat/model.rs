use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    Vision(Vec<VisionMessageContent>),
}

#[derive(Serialize, Deserialize)]
pub struct VisionMessageContent {
    #[serde(rename = "type")]
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<ImageUrl>,
}

#[derive(Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: MessageContent,
}

#[derive(Serialize, Deserialize, PartialEq)]
pub enum Role {
    #[serde(rename = "system", alias = "developer")]
    System,
    #[serde(rename = "user", alias = "human")]
    User,
    #[serde(rename = "assistant", alias = "ai")]
    Assistant,
}

#[derive(Serialize)]
pub struct ChatResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub choices: Vec<Choice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

#[derive(Serialize)]
pub struct Choice {
    pub index: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<Delta>,
    pub finish_reason: Option<String>,
}

#[derive(Serialize)]
pub struct Delta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[derive(Serialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

// 模型定义
#[derive(Serialize, Clone)]
pub struct Model {
    pub id: String,
    pub created: &'static i64,
    pub object: &'static str,
    pub owned_by: &'static str,
}

impl PartialEq for Model {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

use super::constant::{Models, USAGE_CHECK_MODELS};
use crate::app::model::{AppConfig, UsageCheck};

impl Model {
    pub fn is_usage_check(model_id: &String, usage_check: Option<UsageCheck>) -> bool {
        match usage_check.unwrap_or(AppConfig::get_usage_check()) {
            UsageCheck::None => false,
            UsageCheck::Default => USAGE_CHECK_MODELS.contains(&model_id.as_str()),
            UsageCheck::All => true,
            UsageCheck::Custom(models) => models.contains(model_id),
        }
    }
}

#[derive(Serialize)]
pub struct ModelsResponse {
    pub object: &'static str,
    pub data: Arc<Vec<Model>>,
}

impl ModelsResponse {
    pub(super) fn new(data: Arc<Vec<Model>>) -> Self {
        Self {
            object: "list",
            data,
        }
    }

    pub(super) fn with_default_models() -> Self {
        Self::new(Models::to_arc())
    }
}
