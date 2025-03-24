macro_rules! def_pub_const {
    // 单个常量定义
    // ($name:ident, $value:expr) => {
    //     pub const $name: &'static str = $value;
    // };

    // 批量常量定义
    ($($name:ident => $value:expr),+ $(,)?) => {
        $(
            pub const $name: &'static str = $value;
        )+
    };
}

pub const COMMA: char = ',';

// Package related constants
def_pub_const!(
    PKG_VERSION => env!("CARGO_PKG_VERSION")
    // PKG_NAME => env!("CARGO_PKG_NAME"),
    // PKG_DESCRIPTION => env!("CARGO_PKG_DESCRIPTION"),
    // PKG_AUTHORS => env!("CARGO_PKG_AUTHORS"),
    // PKG_REPOSITORY => env!("CARGO_PKG_REPOSITORY")
);

// Basic string constants
def_pub_const!(
    EMPTY_STRING => "",
    COMMA_STRING => ","
);

// Route related constants
def_pub_const!(
    ROUTE_ROOT_PATH => "/",
    ROUTE_HEALTH_PATH => "/health",
    ROUTE_GET_HASH => "/get-hash",
    ROUTE_GET_CHECKSUM => "/get-checksum",
    ROUTE_GET_TIMESTAMP_HEADER => "/get-tsheader",
    ROUTE_USER_INFO_PATH => "/userinfo",
    ROUTE_API_PATH => "/api",
    ROUTE_LOGS_PATH => "/logs",
    ROUTE_CONFIG_PATH => "/config",
    ROUTE_TOKENS_PATH => "/tokens",
    ROUTE_TOKENS_GET_PATH => "/tokens/get",
    ROUTE_TOKENS_UPDATE_PATH => "/tokens/update",
    ROUTE_TOKENS_ADD_PATH => "/tokens/add",
    ROUTE_TOKENS_DELETE_PATH => "/tokens/delete",
    ROUTE_TOKEN_TAGS_UPDATE_PATH => "/tokens/tags/update",
    ROUTE_ENV_EXAMPLE_PATH => "/env-example",
    ROUTE_STATIC_PATH => "/static/{path}",
    ROUTE_SHARED_STYLES_PATH => "/static/shared-styles.css",
    ROUTE_SHARED_JS_PATH => "/static/shared.js",
    ROUTE_ABOUT_PATH => "/about",
    ROUTE_README_PATH => "/readme",
    ROUTE_BASIC_CALIBRATION_PATH => "/basic-calibration",
    ROUTE_BUILD_KEY_PATH => "/build-key"
);

// def_pub_const!(DEFAULT_TOKEN_LIST_FILE_NAME => ".tokens");

// Status constants
def_pub_const!(
    STATUS_PENDING => "pending",
    STATUS_SUCCESS => "success",
    STATUS_FAILED => "failed"
);

// Header constants
def_pub_const!(
    HEADER_NAME_GHOST_MODE => "x-ghost-mode"
);

// Boolean constants
def_pub_const!(
    TRUE => "true",
    FALSE => "false"
);

// Content type constants
def_pub_const!(
    CONTENT_TYPE_PROTO => "application/proto",
    CONTENT_TYPE_CONNECT_PROTO => "application/connect+proto",
    CONTENT_TYPE_TEXT_HTML_WITH_UTF8 => "text/html;charset=utf-8",
    CONTENT_TYPE_TEXT_PLAIN_WITH_UTF8 => "text/plain;charset=utf-8",
    CONTENT_TYPE_TEXT_CSS_WITH_UTF8 => "text/css;charset=utf-8",
    CONTENT_TYPE_TEXT_JS_WITH_UTF8 => "text/javascript;charset=utf-8"
);

// Authorization constants
def_pub_const!(
    AUTHORIZATION_BEARER_PREFIX => "Bearer "
);

// Cursor related constants
def_pub_const!(
    CURSOR_API2_HOST => "api2.cursor.sh",
    CURSOR_HOST => "www.cursor.com",
    CURSOR_SETTINGS_URL => "https://www.cursor.com/settings"
);

// Object type constants
def_pub_const!(
    OBJECT_CHAT_COMPLETION => "chat.completion",
    OBJECT_CHAT_COMPLETION_CHUNK => "chat.completion.chunk"
);

// def_pub_const!(
//     CURSOR_API2_STREAM_CHAT => "StreamChat",
//     CURSOR_API2_GET_USER_INFO => "GetUserInfo"
// );

// Finish reason constants
def_pub_const!(
    FINISH_REASON_STOP => "stop"
);

// Error message constants
def_pub_const!(
    ERR_INVALID_PATH => "无效的路径"
);

// def_pub_const!(ERR_CHECKSUM_NO_GOOD => "checksum no good");
