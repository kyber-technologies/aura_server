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
    pub fn internal(message: impl ToSmolStr) -> Self {
        Self {
            code: ErrorCode::Internal,
            message: message.to_smolstr(),
        }
    }

    pub fn unauthorized(message: impl ToSmolStr) -> Self {
        Self {
            code: ErrorCode::Unauthorized,
            message: message.to_smolstr(),
        }
    }

    pub fn not_found(message: impl ToSmolStr) -> Self {
        Self {
            code: ErrorCode::NotFound,
            message: message.to_smolstr(),
        }
    }

    pub fn already_exists(message: impl ToSmolStr) -> Self {
        Self {
            code: ErrorCode::AlreadyExists,
            message: message.to_smolstr(),
        }
    }

    pub fn invalid_format(message: impl ToSmolStr) -> Self {
        Self {
            code: ErrorCode::InvalidFormat,
            message: message.to_smolstr(),
        }
    }

    pub fn restricted(message: impl ToSmolStr) -> Self {
        Self {
            code: ErrorCode::Restricted,
            message: message.to_smolstr(),
        }
    }

    pub fn unwanted(message: impl ToSmolStr) -> Self {
        Self {
            code: ErrorCode::Unwanted,
            message: message.to_smolstr(),
        }
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
            ErrorCode::Unwanted => "UNWANTED",
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code_name(), self.message)
    }
}

impl std::error::Error for Error {}

impl From<aura_rust::common::v1::Error> for Error {
    fn from(value: aura_rust::common::v1::Error) -> Self {
        Self {
            code: value.code(),
            message: value.message.to_smolstr(),
        }
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
        Self::internal(format!("Database Connection Error: {value}"))
    }
}

impl From<diesel::result::Error> for Error {
    fn from(value: diesel::result::Error) -> Self {
        Self::internal(format!("Database Error: {value}"))
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::invalid_format(format!("JSON Error: {value}"))
    }
}
