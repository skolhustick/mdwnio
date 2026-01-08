use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

/// Error types for mdwn.io - designed to be LLM-friendly (short, parseable, actionable)
#[derive(Error, Debug)]
pub enum MdwnError {
    #[error("INVALID_URL: {0}")]
    InvalidUrl(String),

    #[error("BLOCKED_URL: URL points to a private/internal address")]
    BlockedUrl,

    #[error("FETCH_FAILED: {0}")]
    FetchFailed(String),

    #[error("TIMEOUT: Request timed out after {0} seconds")]
    Timeout(u64),

    #[error("NOT_FOUND: Upstream returned 404")]
    NotFound,

    #[error("FORBIDDEN: Upstream returned 403")]
    Forbidden,

    #[error("NO_MARKDOWN: {0}")]
    NoMarkdown(String),

    #[error("UNSUPPORTED_TYPE: Content-Type '{0}' is not supported")]
    UnsupportedType(String),

    #[error("TOO_LARGE: Content exceeds {0} byte limit")]
    TooLarge(usize),

    #[error("PARSE_ERROR: {0}")]
    ParseError(String),

    #[error("INTERNAL_ERROR: {0}")]
    Internal(String),
}

impl IntoResponse for MdwnError {
    fn into_response(self) -> Response {
        let status = match &self {
            MdwnError::InvalidUrl(_) => StatusCode::BAD_REQUEST,
            MdwnError::BlockedUrl => StatusCode::FORBIDDEN,
            MdwnError::FetchFailed(_) => StatusCode::BAD_GATEWAY,
            MdwnError::Timeout(_) => StatusCode::GATEWAY_TIMEOUT,
            MdwnError::NotFound => StatusCode::NOT_FOUND,
            MdwnError::Forbidden => StatusCode::FORBIDDEN,
            MdwnError::NoMarkdown(_) => StatusCode::NOT_FOUND,
            MdwnError::UnsupportedType(_) => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            MdwnError::TooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,
            MdwnError::ParseError(_) => StatusCode::UNPROCESSABLE_ENTITY,
            MdwnError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        // LLM-friendly error format: short, parseable
        let body = format!("ERROR: {}\n", self);

        (status, body).into_response()
    }
}

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, MdwnError>;
