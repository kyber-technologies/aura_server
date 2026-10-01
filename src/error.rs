use aura_rust::common::v1::{
    AlreadyExistsError, InternalError, InvalidFormatError, NotFoundError, RateLimitError,
    RestrictedError, UnauthorizedError, UnwantedError,
};
use diesel_async::pooled_connection::deadpool::PoolError;
use smol_str::{SmolStr, ToSmolStr, format_smolstr};
use std::fmt::{Debug, Display, Formatter};
use std::time::Duration;

pub type ErrorType = aura_rust::common::v1::error::Type;

#[derive(Clone, Debug)]
pub struct Error {
    pub message: SmolStr,
    pub ty: ErrorType,
}

impl Error {
    pub fn internal(message: impl ToSmolStr) -> Self {
        Self {
            message: message.to_smolstr(),
            ty: ErrorType::Internal(InternalError {}),
        }
    }

    pub fn unauthorized(message: impl ToSmolStr) -> Self {
        Self {
            message: message.to_smolstr(),
            ty: ErrorType::Unauthorized(UnauthorizedError {}),
        }
    }

    pub fn not_found(message: impl ToSmolStr) -> Self {
        Self {
            message: message.to_smolstr(),
            ty: ErrorType::NotFound(NotFoundError {}),
        }
    }

    pub fn already_exists(message: impl ToSmolStr) -> Self {
        Self {
            message: message.to_smolstr(),
            ty: ErrorType::AlreadyExists(AlreadyExistsError {}),
        }
    }

    pub fn invalid_format(message: impl ToSmolStr) -> Self {
        Self {
            message: message.to_smolstr(),
            ty: ErrorType::InvalidFormat(InvalidFormatError {}),
        }
    }

    pub fn restricted(message: impl ToSmolStr) -> Self {
        Self {
            message: message.to_smolstr(),
            ty: ErrorType::Restricted(RestrictedError {}),
        }
    }

    pub fn unwanted(message: impl ToSmolStr) -> Self {
        Self {
            message: message.to_smolstr(),
            ty: ErrorType::Unwanted(UnwantedError {}),
        }
    }

    pub fn rate_limit(message: impl ToSmolStr, duration: Duration) -> Self {
        Self {
            message: message.to_smolstr(),
            ty: ErrorType::RateLimit(RateLimitError {
                try_again_in: Some(
                    aura_rust::types::Duration::try_from(duration).unwrap_or_else(|err| {
                        tracing::warn!("Failed to convert to gRPC duration: {err}");
                        aura_rust::types::Duration::default()
                    }),
                ),
            }),
        }
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: {}",
            match self.ty {
                ErrorType::Internal(_) => "INTERNAL",
                ErrorType::Unauthorized(_) => "UNAUTHORIZED",
                ErrorType::NotFound(_) => "NOT_FOUND",
                ErrorType::AlreadyExists(_) => "ALREADY_EXISTS",
                ErrorType::InvalidFormat(_) => "INVALID_FORMAT",
                ErrorType::Restricted(_) => "RESTRICTED",
                ErrorType::Unwanted(_) => "UNWANTED",
                ErrorType::RateLimit(_) => "RATE_LIMIT",
            },
            self.message
        )
    }
}

impl std::error::Error for Error {}

impl From<aura_rust::common::v1::Error> for Error {
    fn from(value: aura_rust::common::v1::Error) -> Self {
        if let Some(ty) = value.r#type {
            Self {
                message: value.message.to_smolstr(),
                ty,
            }
        } else {
            Self {
                message: format_smolstr!("No error type provided for message: {}", value.message),
                ty: ErrorType::InvalidFormat(InvalidFormatError {}),
            }
        }
    }
}

impl From<Error> for aura_rust::common::v1::Error {
    fn from(value: Error) -> aura_rust::common::v1::Error {
        aura_rust::common::v1::Error {
            message: value.message.to_string(),
            r#type: Some(value.ty),
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
