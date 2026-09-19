use aura_rust::common::v1::ErrorCode;
use diesel_async::pooled_connection::deadpool::PoolError;
use smol_str::{SmolStr, ToSmolStr};
use std::fmt::{Debug, Display, Formatter};

#[derive(Clone, Debug)]
pub struct Error {
    pub code: ErrorCode,
    pub message: SmolStr,
}

impl Error {
    pub fn new(code: ErrorCode, message: impl ToSmolStr) -> Self {
        Self {
            code,
            message: message.to_smolstr(),
        }
    }

    pub fn invalid_format() -> Self {
        ErrorCode::InvalidFormat.into()
    }

    pub fn internal(message: impl ToSmolStr) -> Self {
        Self::new(ErrorCode::Internal, message)
    }

    pub fn not_found(message: impl ToSmolStr) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    pub fn code_name(&self) -> &'static str {
        match self.code {
            ErrorCode::Unspecified => "UNSPECIFIED",
            ErrorCode::Internal => "INTERNAL",
            ErrorCode::Unauthorized => "UNAUTHORIZED",
            ErrorCode::NotFound => "NOT_FOUND",
            ErrorCode::AlreadyExists => "ALREADY_EXISTS",
            ErrorCode::InvalidFormat => "INVALID_FORMAT",
            ErrorCode::Restricted => "RESTRICTED",
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code_name(), self.message)
    }
}

impl std::error::Error for Error {}

impl From<ErrorCode> for Error {
    fn from(value: ErrorCode) -> Self {
        Self::new(
            value,
            SmolStr::new_static(match value {
                ErrorCode::Unspecified => "An unspecified error happened. Please report this!",
                ErrorCode::Internal => "An internal error happened. Please report this!",
                ErrorCode::Unauthorized => "You are not authorized to do this.",
                ErrorCode::NotFound => "The target entity could not be found.",
                ErrorCode::AlreadyExists => "The target entity already exists",
                ErrorCode::InvalidFormat => {
                    "An invalid message was given. Are you using the latest API?"
                }
                ErrorCode::Restricted => "You are not permitted to do that.",
            }),
        )
    }
}

impl From<aura_rust::common::v1::Error> for Error {
    fn from(value: aura_rust::common::v1::Error) -> Self {
        Self::new(value.code(), value.message)
    }
}

impl From<Error> for aura_rust::common::v1::Error {
    fn from(value: Error) -> aura_rust::common::v1::Error {
        aura_rust::common::v1::Error {
            code: value.code as i32,
            message: value.message.to_string(),
        }
    }
}

impl From<PoolError> for Error {
    fn from(value: PoolError) -> Self {
        Self::new(
            ErrorCode::Internal,
            format!("Database Connection Error: {value}"),
        )
    }
}

impl From<diesel::result::Error> for Error {
    fn from(value: diesel::result::Error) -> Self {
        Self::new(ErrorCode::Internal, format!("Database Error: {value}"))
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::new(ErrorCode::InvalidFormat, format!("JSON Error: {value}"))
    }
}
