use serde::{Deserialize, Serialize};

use crate::{app::constant::COMMA, chat::constant::Models};

#[derive(Deserialize)]
pub struct BuildKeyRequest {
    pub auth_token: String,
    #[serde(default)]
    pub disable_vision: Option<bool>,
    #[serde(default)]
    pub enable_slow_pool: Option<bool>,
    #[serde(default)]
    pub usage_check_models: Option<UsageCheckModelConfig>,
    #[serde(default)]
    pub include_web_references: Option<bool>,
}
pub struct UsageCheckModelConfig {
    pub model_type: UsageCheckModelType,
    pub model_ids: Vec<String>,
}

impl<'de> Deserialize<'de> for UsageCheckModelConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            #[serde(rename = "type")]
            model_type: UsageCheckModelType,
            #[serde(default)]
            model_ids: String,
        }

        let helper = Helper::deserialize(deserializer)?;

        let model_ids = if helper.model_ids.is_empty() {
            Vec::new()
        } else {
            helper
                .model_ids
                .split(COMMA)
                .filter_map(|model| {
                    let model = model.trim();
                    Models::find_id(model)
                })
                .collect()
        };

        Ok(UsageCheckModelConfig {
            model_type: helper.model_type,
            model_ids,
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UsageCheckModelType {
    Default,
    Disabled,
    All,
    Custom,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildKeyResponse {
    Key(String),
    Error(String),
}
