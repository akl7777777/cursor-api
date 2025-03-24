use axum::{
    body::Body,
    response::{IntoResponse, Response},
};
use reqwest::header::CONTENT_TYPE;

use crate::{
    AppConfig, PageContent,
    app::constant::{
        CONTENT_TYPE_TEXT_HTML_WITH_UTF8, CONTENT_TYPE_TEXT_PLAIN_WITH_UTF8, ROUTE_API_PATH,
    },
};

pub async fn handle_api_page() -> impl IntoResponse {
    match AppConfig::get_page_content(ROUTE_API_PATH).unwrap_or_default() {
        PageContent::Default => Response::builder()
            .header(CONTENT_TYPE, CONTENT_TYPE_TEXT_HTML_WITH_UTF8)
            .body(Body::from(include_str!("../../../static/api.min.html")))
            .unwrap(),
        PageContent::Text(content) => Response::builder()
            .header(CONTENT_TYPE, CONTENT_TYPE_TEXT_PLAIN_WITH_UTF8)
            .body(Body::from(content))
            .unwrap(),
        PageContent::Html(content) => Response::builder()
            .header(CONTENT_TYPE, CONTENT_TYPE_TEXT_HTML_WITH_UTF8)
            .body(Body::from(content))
            .unwrap(),
    }
}
