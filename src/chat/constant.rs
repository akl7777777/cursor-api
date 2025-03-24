use parking_lot::RwLock;
use std::{sync::Arc, time::{Duration, Instant}};

use super::model::Model;

macro_rules! def_pub_const {
    // 单个常量定义分支
    ($name:ident, $value:expr) => {
        pub const $name: &'static str = $value;
    };

    // 批量定义分支
    ($($name:ident => $value:expr),+ $(,)?) => {
        $(
            pub const $name: &'static str = $value;
        )+
    };
}

// 错误信息
def_pub_const!(
    ERR_UNSUPPORTED_GIF => "不支持动态 GIF",
    ERR_UNSUPPORTED_IMAGE_FORMAT => "不支持的图片格式，仅支持 PNG、JPEG、WEBP 和非动态 GIF",
    ERR_NODATA => "No data",
);

// 系统常量
pub const MODEL_OBJECT: &str = "model";
pub const CREATED: &i64 = &1706659200;

// AI 服务商
def_pub_const!(
    ANTHROPIC => "anthropic",
    CURSOR => "cursor",
    GOOGLE => "google",
    OPENAI => "openai",
    DEEPSEEK => "deepseek",
    XAI => "xai",
    UNKNOWN => "unknown",
);

// AI 模型
def_pub_const!(
    // Anthropic 模型
    CLAUDE_3_OPUS => "claude-3-opus",
    CLAUDE_3_5_SONNET => "claude-3.5-sonnet",
    CLAUDE_3_HAIKU_200K => "claude-3-haiku-200k",
    CLAUDE_3_5_SONNET_200K => "claude-3-5-sonnet-200k",
    CLAUDE_3_5_SONNET_20241022 => "claude-3-5-sonnet-20241022",
    CLAUDE_3_5_HAIKU => "claude-3.5-haiku",

    // OpenAI 模型
    GPT_4 => "gpt-4",
    GPT_4O => "gpt-4o",
    GPT_3_5_TURBO => "gpt-3.5-turbo",
    GPT_4_TURBO_2024_04_09 => "gpt-4-turbo-2024-04-09",
    GPT_4O_128K => "gpt-4o-128k",
    GPT_4O_MINI => "gpt-4o-mini",
    O1_MINI => "o1-mini",
    O1_PREVIEW => "o1-preview",
    O1 => "o1",
    O3_MINI => "o3-mini",

    // Cursor 模型
    CURSOR_FAST => "cursor-fast",
    CURSOR_SMALL => "cursor-small",

    // Google 模型
    GEMINI_1_5_FLASH_500K => "gemini-1.5-flash-500k",
    GEMINI_EXP_1206 => "gemini-exp-1206",
    GEMINI_2_0_PRO_EXP => "gemini-2.0-pro-exp",
    GEMINI_2_0_FLASH_THINKING_EXP => "gemini-2.0-flash-thinking-exp",
    GEMINI_2_0_FLASH => "gemini-2.0-flash",

    // Deepseek 模型
    DEEPSEEK_V3 => "deepseek-v3",
    DEEPSEEK_R1 => "deepseek-r1",

    // XAI 模型
    GROK_2 => "grok-2",
);

macro_rules! create_models {
    ($($model:expr => $owner:expr),* $(,)?) => {
        static INSTANCE: std::sync::LazyLock<RwLock<Models>> = std::sync::LazyLock::new(|| {
            RwLock::new(Models {
                models: Arc::new(vec![
                    $(
                        Model {
                            id: $model.into(),
                            created: CREATED,
                            object: MODEL_OBJECT,
                            owned_by: $owner,
                        },
                    )*
                ]),
                last_update: Instant::now() - Duration::from_secs(30 * 60),
            })
        });
    };
}

pub struct Models {
    pub models: Arc<Vec<Model>>,
    last_update: Instant,
}

impl Models {
    // 返回读锁
    pub fn read() -> parking_lot::RwLockReadGuard<'static, Models> {
        INSTANCE.read()
    }

    // 返回 Arc 的克隆
    pub fn to_arc() -> Arc<Vec<Model>> {
        INSTANCE.read().models.clone()
    }

    // 克隆所有模型
    // pub fn cloned() -> Vec<Model> {
    //     INSTANCE.read().models.as_ref().clone()
    // }

    // 检查模型是否存在
    pub fn exists(model_id: &str) -> bool {
        Self::read().models.iter().any(|m| m.id == model_id)
    }

    // 查找模型并返回其 ID
    pub fn find_id(model: &str) -> Option<String> {
        Self::read()
            .models
            .iter()
            .find(|m| m.id == model)
            .map(|m| m.id.clone())
    }

    // 返回所有模型 ID 的列表
    pub fn ids() -> Vec<String> {
        Self::read()
            .models
            .iter()
            .map(|m| m.id.clone())
            .collect()
    }

    // 写入方法
    pub fn update(new_models: Vec<Model>) -> Result<(), &'static str> {
        if new_models.is_empty() {
            return Err("Models list cannot be empty");
        }

        let mut data = INSTANCE.write();

        // 检查时间间隔（30分钟）
        if data.last_update.elapsed() < Duration::from_secs(30 * 60) {
            return Err("Cannot update models more frequently than every 30 minutes");
        }

        // 检查内容是否有变化
        if *data.models == new_models {
            return Err("No changes in models");
        }

        // 更新数据和时间戳
        data.models = Arc::new(new_models);
        data.last_update = Instant::now();

        Ok(())
    }
}

// macro_rules! count {
//     () => (0);
//     (($id:expr, $owner:expr) $( ($id2:expr, $owner2:expr) )*) => (1 + count!($( ($id2, $owner2) )*));
// }

create_models!(
    CLAUDE_3_5_SONNET => ANTHROPIC,
    GPT_4 => OPENAI,
    GPT_4O => OPENAI,
    CLAUDE_3_OPUS => ANTHROPIC,
    CURSOR_FAST => CURSOR,
    CURSOR_SMALL => CURSOR,
    GPT_3_5_TURBO => OPENAI,
    GPT_4_TURBO_2024_04_09 => OPENAI,
    GPT_4O_128K => OPENAI,
    GEMINI_1_5_FLASH_500K => GOOGLE,
    CLAUDE_3_HAIKU_200K => ANTHROPIC,
    CLAUDE_3_5_SONNET_200K => ANTHROPIC,
    GPT_4O_MINI => OPENAI,
    O1_MINI => OPENAI,
    O1_PREVIEW => OPENAI,
    O1 => OPENAI,
    CLAUDE_3_5_HAIKU => ANTHROPIC,
    GEMINI_2_0_PRO_EXP => GOOGLE,
    GEMINI_2_0_FLASH_THINKING_EXP => GOOGLE,
    GEMINI_2_0_FLASH => GOOGLE,
    DEEPSEEK_V3 => DEEPSEEK,
    DEEPSEEK_R1 => DEEPSEEK,
    O3_MINI => OPENAI,
    GROK_2 => XAI,
);

pub const USAGE_CHECK_MODELS: [&str; 11] = [
    CLAUDE_3_5_SONNET_20241022,
    CLAUDE_3_5_SONNET,
    GEMINI_EXP_1206,
    GPT_4,
    GPT_4_TURBO_2024_04_09,
    GPT_4O,
    CLAUDE_3_5_HAIKU,
    GPT_4O_128K,
    GEMINI_1_5_FLASH_500K,
    CLAUDE_3_HAIKU_200K,
    CLAUDE_3_5_SONNET_200K,
];

pub const LONG_CONTEXT_MODELS: [&str; 4] = [
    GPT_4O_128K,
    GEMINI_1_5_FLASH_500K,
    CLAUDE_3_HAIKU_200K,
    CLAUDE_3_5_SONNET_200K,
];
