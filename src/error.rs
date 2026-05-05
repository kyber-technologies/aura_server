use aura_rust::common::v1::ErrorCode;
use std::fmt::{Debug, Display, Formatter};

#[derive(Debug)]
pub struct Error(aura_rust::common::v1::Error);

impl Error {
    pub fn new(code: ErrorCode, message: impl ToString) -> Self {
        Self(aura_rust::common::v1::Error {
            code: code as i32,
            message: message.to_string(),
        })
    }

    pub fn invalid_argument() -> Self {
        ErrorCode::InvalidFormat.into()
    }

    pub fn code(&self) -> ErrorCode {
        ErrorCode::try_from(self.0.code).unwrap_or(ErrorCode::Internal)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: {}",
            ErrorCode::try_from(self.0.code)
                .map(|code| code.as_str_name())
                .unwrap_or("INVALID_ERROR_CODE"),
            self.0.message
        )
    }
}

impl std::error::Error for Error {}

impl From<ErrorCode> for Error {
    fn from(value: ErrorCode) -> Self {
        Self::new(
            value,
            match value {
                ErrorCode::Unspecified => "An unspecified error happened",
                ErrorCode::Internal => "An internal error happened",
                ErrorCode::Unauthorized => "You are not authorized to do this",
                ErrorCode::NotFound => "The requested item could not be found",
                ErrorCode::AlreadyExists => "The requested item already exists",
                ErrorCode::InvalidFormat => "An invalid message was given",
            },
        )
    }
}

impl From<aura_rust::common::v1::Error> for Error {
    fn from(value: aura_rust::common::v1::Error) -> Self {
        Self(value)
    }
}

impl Into<aura_rust::common::v1::Error> for Error {
    fn into(self) -> aura_rust::common::v1::Error {
        self.0
    }
}

impl From<surrealdb::Error> for Error {
    fn from(value: surrealdb::Error) -> Self {
        Self::new(ErrorCode::Internal, value)
    }
}
