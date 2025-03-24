use super::constant::{COMMA, CURSOR_API2_HOST, CURSOR_HOST, EMPTY_STRING};
use crate::common::utils::{
    parse_ascii_char_from_env, parse_bool_from_env, parse_string_from_env, parse_usize_from_env,
};
use chrono::Local;
use std::{path::PathBuf, sync::LazyLock};
use tokio::sync::{Mutex, OnceCell};

macro_rules! def_pub_static {
    // 基础版本：直接存储 String
    ($name:ident, $value:expr) => {
        pub static $name: LazyLock<String> = LazyLock::new(|| $value);
    };

    // 环境变量版本
    ($name:ident, env: $env_key:expr, default: $default:expr) => {
        pub static $name: LazyLock<String> =
            LazyLock::new(|| parse_string_from_env($env_key, $default).trim().to_string());
    };
}

// macro_rules! def_pub_static_getter {
//     ($name:ident) => {
//         paste::paste! {
//             pub fn [<get_ $name:lower>]() -> String {
//                 (*$name).clone()
//             }
//         }
//     };
// }

def_pub_static!(ROUTE_PREFIX, env: "ROUTE_PREFIX", default: EMPTY_STRING);
def_pub_static!(AUTH_TOKEN, env: "AUTH_TOKEN", default: EMPTY_STRING);
def_pub_static!(ROUTE_MODELS_PATH, format!("{}/v1/models", *ROUTE_PREFIX));
def_pub_static!(
    ROUTE_CHAT_PATH,
    format!("{}/v1/chat/completions", *ROUTE_PREFIX)
);

pub static START_TIME: LazyLock<chrono::DateTime<Local>> = LazyLock::new(Local::now);

pub fn get_start_time() -> chrono::DateTime<Local> {
    *START_TIME
}

def_pub_static!(DEFAULT_INSTRUCTIONS, env: "DEFAULT_INSTRUCTIONS", default: "Respond in Chinese by default");

def_pub_static!(REVERSE_PROXY_HOST, env: "REVERSE_PROXY_HOST", default: EMPTY_STRING);

const DEFAULT_KEY_PREFIX: &str = "sk-";

pub static KEY_PREFIX: LazyLock<String> = LazyLock::new(|| {
    let value = parse_string_from_env("KEY_PREFIX", DEFAULT_KEY_PREFIX)
        .trim()
        .to_string();
    if value.is_empty() {
        DEFAULT_KEY_PREFIX.to_string()
    } else {
        value
    }
});

pub static KEY_PREFIX_LEN: LazyLock<usize> = LazyLock::new(|| KEY_PREFIX.len());

pub static TOKEN_DELIMITER: LazyLock<char> = LazyLock::new(|| {
    let delimiter = parse_ascii_char_from_env("TOKEN_DELIMITER", COMMA);
    if delimiter.is_ascii_alphabetic()
        || delimiter.is_ascii_digit()
        || delimiter == '+'
        || delimiter == '/'
    {
        COMMA
    } else {
        delimiter
    }
});

pub static USE_COMMA_DELIMITER: LazyLock<bool> = LazyLock::new(|| {
    let enable = parse_bool_from_env("USE_COMMA_DELIMITER", true);
    if enable && *TOKEN_DELIMITER == COMMA {
        false
    } else {
        enable
    }
});

pub static USE_REVERSE_PROXY: LazyLock<bool> = LazyLock::new(|| !REVERSE_PROXY_HOST.is_empty());

macro_rules! def_cursor_api_url {
    ($name:ident, $api_host:expr, $path:expr) => {
        pub static $name: LazyLock<String> = LazyLock::new(|| {
            let host = if *USE_REVERSE_PROXY {
                &*REVERSE_PROXY_HOST
            } else {
                $api_host
            };
            format!("https://{}{}", host, $path)
        });
    };
}

def_cursor_api_url!(
    CURSOR_API2_CHAT_URL,
    CURSOR_API2_HOST,
    "/aiserver.v1.AiService/StreamChat"
);

def_cursor_api_url!(
    CURSOR_API2_CHAT_WEB_URL,
    CURSOR_API2_HOST,
    "/aiserver.v1.AiService/StreamChatWeb"
);

def_cursor_api_url!(
    CURSOR_API2_CHAT_MODELS_URL,
    CURSOR_API2_HOST,
    "/aiserver.v1.AiService/AvailableModels"
);

def_cursor_api_url!(
    CURSOR_API2_STRIPE_URL,
    CURSOR_API2_HOST,
    "/auth/full_stripe_profile"
);

def_cursor_api_url!(CURSOR_USAGE_API_URL, CURSOR_HOST, "/api/usage");

def_cursor_api_url!(CURSOR_USER_API_URL, CURSOR_HOST, "/api/auth/me");

static DATA_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    let data_dir = parse_string_from_env("DATA_DIR", "data");
    let path = std::env::current_exe()
        .ok()
        .and_then(|exe_path| exe_path.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join(data_dir);
    if !path.exists() {
        std::fs::create_dir_all(&path).expect("无法创建数据目录");
    }
    path
});

pub(super) static CONFIG_FILE_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| DATA_DIR.join("config.bin"));

pub(super) static LOGS_FILE_PATH: LazyLock<PathBuf> = LazyLock::new(|| DATA_DIR.join("logs.bin"));

pub(super) static TOKENS_FILE_PATH: LazyLock<PathBuf> =
    LazyLock::new(|| DATA_DIR.join("tokens.bin"));

pub static DEBUG: LazyLock<bool> = LazyLock::new(|| parse_bool_from_env("DEBUG", false));

// 使用环境变量 "DEBUG_LOG_FILE" 来指定日志文件路径，默认值为 "debug.log"
static DEBUG_LOG_FILE: LazyLock<String> =
    LazyLock::new(|| parse_string_from_env("DEBUG_LOG_FILE", "debug.log"));

// 使用 OnceCell 结合 Mutex 来异步初始化 LOG_FILE
static LOG_FILE: OnceCell<Mutex<tokio::fs::File>> = OnceCell::const_new();

pub(crate) async fn get_log_file() -> &'static Mutex<tokio::fs::File> {
    LOG_FILE
        .get_or_init(|| async {
            Mutex::new(
                tokio::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&*DEBUG_LOG_FILE)
                    .await
                    .expect("无法打开日志文件"),
            )
        })
        .await
}

#[macro_export]
macro_rules! debug_println {
    ($($arg:tt)*) => {
        if *$crate::app::lazy::DEBUG {
            let time = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let log_message = format!("{} - {}", time, format!($($arg)*));
            use tokio::io::AsyncWriteExt as _;

            // 使用 tokio 的 spawn 在后台异步写入日志
            tokio::spawn(async move {
                let log_file = $crate::app::lazy::get_log_file().await;
                // 使用 MutexGuard 获取可变引用
                let mut file = log_file.lock().await;
                if let Err(err) = file.write_all(log_message.as_bytes()).await {
                    eprintln!("写入日志文件失败: {}", err);
                }
                if let Err(err) = file.write_all(b"\n").await {
                    eprintln!("写入换行符失败: {}", err);
                }
                // 可以选择在写入失败时 panic，或者忽略
                // panic!("写入日志文件失败: {}", err);
            });
        }
    };
}

pub static REQUEST_LOGS_LIMIT: LazyLock<usize> =
    LazyLock::new(|| std::cmp::min(parse_usize_from_env("REQUEST_LOGS_LIMIT", 100), 2000));

pub static IS_UNLIMITED_REQUEST_LOGS: LazyLock<bool> = LazyLock::new(|| *REQUEST_LOGS_LIMIT == 0);

pub static SERVICE_TIMEOUT: LazyLock<u64> = LazyLock::new(|| {
    let timeout = parse_usize_from_env("SERVICE_TIMEOUT", 30);
    u64::try_from(timeout).map(|t| t.min(600)).unwrap_or(30)
});
