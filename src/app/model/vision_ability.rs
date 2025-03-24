use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq)]
pub enum VisionAbility {
    #[serde(rename = "none", alias = "disabled")]
    None,
    #[serde(rename = "base64", alias = "base64-only")]
    Base64,
    #[serde(rename = "all", alias = "base64-http")]
    All,
}

impl VisionAbility {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "none" | "disabled" => Self::None,
            "base64" | "base64-only" => Self::Base64,
            "all" | "base64-http" => Self::All,
            _ => Self::default(),
        }
    }

    pub fn is_none(&self) -> bool {
        matches!(self, VisionAbility::None)
    }
}

impl Default for VisionAbility {
    fn default() -> Self {
        Self::Base64
    }
}
