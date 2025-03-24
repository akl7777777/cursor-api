use axum::{
    extract::Query,
    http::{HeaderMap, header::CONTENT_TYPE},
    response::{IntoResponse as _, Response},
};
use serde::Deserialize;

use crate::{
    app::constant::CONTENT_TYPE_TEXT_PLAIN_WITH_UTF8,
    common::utils::{
        generate_checksum_with_default, generate_checksum_with_repair, generate_hash,
        generate_timestamp_header,
    },
};

pub async fn handle_get_hash() -> Response {
    let hash = generate_hash();

    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        CONTENT_TYPE_TEXT_PLAIN_WITH_UTF8.parse().unwrap(),
    );

    (headers, hash).into_response()
}

#[derive(Deserialize)]
pub struct ChecksumQuery {
    #[serde(default)]
    pub checksum: Option<String>,
}

pub async fn handle_get_checksum(Query(query): Query<ChecksumQuery>) -> Response {
    let checksum = match query.checksum {
        None => generate_checksum_with_default(),
        Some(checksum) => generate_checksum_with_repair(&checksum),
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        CONTENT_TYPE_TEXT_PLAIN_WITH_UTF8.parse().unwrap(),
    );

    (headers, checksum).into_response()
}

pub async fn handle_get_timestamp_header() -> Response {
    let timestamp_header = generate_timestamp_header();

    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        CONTENT_TYPE_TEXT_PLAIN_WITH_UTF8.parse().unwrap(),
    );

    (headers, timestamp_header).into_response()
}
