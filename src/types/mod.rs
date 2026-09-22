use crate::error::Error;
use serde::{Deserialize, Serialize};

// TODO: Replace every hashmap mention with this type.
pub type FastMap<K, V> = std::collections::HashMap<K, V, ahash::RandomState>;
pub type FastSet<T> = std::collections::HashSet<T, ahash::RandomState>;
pub type FastDashMap<K, V> = dashmap::DashMap<K, V, ahash::RandomState>;

pub mod chat;
pub mod common;
pub mod posting;
pub mod resource;
pub mod user;

pub trait GrpcDomainType: Sized {
    type Type;

    fn from_grpc(value: Self::Type) -> Result<Self, Error>;
    fn into_grpc(self) -> Result<Self::Type, Error>;
}

pub trait DatabaseDomainType: Sized {
    type Type;

    fn from_db(value: Self::Type) -> Result<Self, Error>;
    fn into_db(self) -> Result<Self::Type, Error>;
}

pub trait JsonDatabaseDomainType: Serialize + for<'a> Deserialize<'a> {}

impl<T: JsonDatabaseDomainType> DatabaseDomainType for T {
    type Type = serde_json::Value;

    fn from_db(value: Self::Type) -> Result<Self, Error> {
        Ok(serde_json::from_value(value)?)
    }

    fn into_db(self) -> Result<Self::Type, Error> {
        Ok(serde_json::to_value(self)?)
    }
}
