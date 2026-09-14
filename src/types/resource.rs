use crate::database::resource as db;
use crate::error::Error;
use crate::types::common::Timestamp;
use crate::types::{DatabaseDomainType, FastMap, GrpcDomainType, JsonDatabaseDomainType};
use aura_rust::resource::v1 as grpc;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResourceId {
    pub namespace: String,
    pub key: String,
}

impl GrpcDomainType for ResourceId {
    type Type = grpc::ResourceId;

    fn from_grpc(value: Self::Type) -> Result<Self, Error> {
        Ok(Self {
            namespace: value.namespace,
            key: value.key,
        })
    }

    fn into_grpc(self) -> Result<Self::Type, Error> {
        Ok(Self::Type {
            namespace: self.namespace,
            key: self.key,
        })
    }
}

impl JsonDatabaseDomainType for ResourceId {}

impl Display for ResourceId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.namespace, self.key)
    }
}

#[derive(Clone, Debug)]
pub struct ResourceDescriptor {
    pub resource_id: ResourceId,
    pub meta: ResourceMeta,
    pub user_id: String,
}

impl DatabaseDomainType for ResourceDescriptor {
    type Type = db::ResourceDescriptor;

    fn from_db(value: Self::Type) -> Result<Self, Error> {
        Ok(Self {
            resource_id: ResourceId {
                namespace: value.namespace,
                key: value.key,
            },
            meta: ResourceMeta::from_db(value.meta)?,
            user_id: value.user_id,
        })
    }

    fn into_db(self) -> Result<Self::Type, Error> {
        Ok(db::ResourceDescriptor {
            namespace: self.resource_id.namespace,
            key: self.resource_id.key,
            meta: self.meta.into_db()?,
            user_id: self.user_id,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResourceMeta {
    pub size: u32,
    pub timestamp: Timestamp,
    pub metadata: FastMap<String, String>,
}

impl JsonDatabaseDomainType for ResourceMeta {}

impl GrpcDomainType for ResourceMeta {
    type Type = grpc::ResourceMeta;

    fn from_grpc(value: Self::Type) -> Result<Self, Error> {
        Ok(Self {
            size: value.size as u32,
            timestamp: Timestamp::from_grpc(value.timestamp.ok_or(Error::invalid_format())?)?,
            metadata: value.metadata.into_iter().collect(),
        })
    }

    fn into_grpc(self) -> Result<Self::Type, Error> {
        Ok(Self::Type {
            size: self.size as i32,
            timestamp: Some(self.timestamp.into_grpc()?),
            metadata: self.metadata.into_iter().collect(),
        })
    }
}
