use crate::error::Error;
use crate::types::GrpcDomainType;
use chrono::{DateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Timestamp(pub DateTime<Utc>);

impl Timestamp {
    pub fn now() -> Self {
        let now = Utc::now();
        Self(
            now.with_nanosecond(now.timestamp_subsec_nanos() / 1_000 * 1_000)
                .expect("valid nanosecond value"),
        )
    }
}

impl GrpcDomainType for Timestamp {
    type Type = aura_rust::types::Timestamp;

    fn from_grpc(value: Self::Type) -> Result<Self, Error> {
        Ok(Self(
            DateTime::from_timestamp(value.seconds, value.nanos as u32)
                .ok_or(Error::internal("Failed to convert timestamp"))?,
        ))
    }

    fn into_grpc(self) -> Result<Self::Type, Error> {
        Ok(Self::Type {
            seconds: self.0.timestamp(),
            nanos: self.0.timestamp_subsec_nanos() as i32,
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Auth {
    pub user_id: String,
    pub exp: u64,
}
