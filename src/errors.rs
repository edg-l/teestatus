use thiserror::Error;

/// Various errors
#[derive(Error, Debug)]
pub enum RequestError {
    /// Error decoding data.
    #[error("decode error '{0}'")]
    DecodeError(#[from] std::str::Utf8Error),
    /// Error parsing data.
    #[error("parse error '{0}'")]
    ParseError(#[from] std::num::ParseIntError),
    /// Errors related to the socket.
    #[error("io error '{0}'")]
    IoError(#[from] std::io::Error),
    /// Missing data.
    #[error("missing data")]
    Missing,
    /// Token validation error.
    #[error("token received by server is invalid")]
    TokenError {
        wanted_extra_token: u16,
        wanted_token: u8,
        received_extra_token: u16,
        received_token: u8,
    },
}

/// A type alias to handle Results with RequestError.
pub type Result<T, V = RequestError> = std::result::Result<T, V>;
