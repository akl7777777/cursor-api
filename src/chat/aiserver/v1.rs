// include!(concat!(env!("OUT_DIR"), "/aiserver.v1.rs"));
include!("v1/aiserver.v1.rs");
use error_details::Error;

impl ErrorDetails {
    pub fn status_code(&self) -> u16 {
        match Error::try_from(self.error) {
            Ok(error) => match error {
                Error::Unspecified => 500,
                Error::BadApiKey
                | Error::BadUserApiKey
                | Error::InvalidAuthId
                | Error::AuthTokenNotFound
                | Error::AuthTokenExpired
                | Error::Unauthorized => 401,
                Error::NotLoggedIn
                | Error::NotHighEnoughPermissions
                | Error::AgentRequiresLogin
                | Error::ProUserOnly
                | Error::TaskNoPermissions => 403,
                Error::NotFound
                | Error::UserNotFound
                | Error::TaskUuidNotFound
                | Error::AgentEngineNotFound
                | Error::GitgraphNotFound
                | Error::FileNotFound => 404,
                Error::FreeUserRateLimitExceeded
                | Error::ProUserRateLimitExceeded
                | Error::OpenaiRateLimitExceeded
                | Error::OpenaiAccountLimitExceeded
                | Error::GenericRateLimitExceeded
                | Error::Gpt4VisionPreviewRateLimit
                | Error::ApiKeyRateLimit => 429,
                Error::BadRequest
                | Error::BadModelName
                | Error::SlashEditFileTooLong
                | Error::FileUnsupported
                | Error::ClaudeImageTooLarge
                | Error::ConversationTooLong => 400,
                Error::Timeout => 504,
                Error::Deprecated
                | Error::FreeUserUsageLimit
                | Error::ProUserUsageLimit
                | Error::ResourceExhausted
                | Error::Openai
                | Error::MaxTokens
                | Error::ApiKeyNotSupported
                | Error::UserAbortedRequest
                | Error::CustomMessage
                | Error::OutdatedClient
                | Error::Debounced
                | Error::RepositoryServiceRepositoryIsNotInitialized => 500,
            },
            Err(_) => 500,
        }
    }

    // pub fn is_expected(&self) -> bool {
    //     self.is_expected.unwrap_or_default()
    // }
}
