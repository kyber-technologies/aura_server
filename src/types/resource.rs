use crate::database::resource as db;
use crate::error::Error;
use crate::types::common::Timestamp;
use crate::types::{DatabaseDomainType, FastMap, GrpcDomainType, JsonDatabaseDomainType};
use aura_rust::resource::v1 as grpc;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceId {
    pub namespace: ResourceNamespace,
    pub key: String,
}

impl ResourceId {
    pub fn default_user_icon() -> Self {
        Self {
            namespace: ResourceNamespace::Aura,
            key: "default_icon.png".to_string(),
        }
    }
}

impl Display for ResourceId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.namespace, self.key)
    }
}

impl GrpcDomainType for ResourceId {
    type Type = grpc::ResourceId;

    fn from_grpc(value: Self::Type) -> Result<Self, Error> {
        Ok(Self {
            namespace: ResourceNamespace::from_grpc(
                value
                    .namespace
                    .ok_or(Error::invalid_format("Namespace not provided"))?,
            )?,
            key: value.key,
        })
    }

    fn into_grpc(self) -> Result<Self::Type, Error> {
        Ok(Self::Type {
            namespace: Some(self.namespace.into_grpc()?),
            key: self.key,
        })
    }
}

impl JsonDatabaseDomainType for ResourceId {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResourceNamespace {
    Aura,
    UserIcon,
    Channel(String),
}

impl Display for ResourceNamespace {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Aura => write!(f, "aura"),
            Self::UserIcon => write!(f, "user_icon"),
            Self::Channel(id) => write!(f, "channel:{}", id),
        }
    }
}

impl GrpcDomainType for ResourceNamespace {
    type Type = grpc::ResourceNamespace;

    fn from_grpc(value: Self::Type) -> Result<Self, Error> {
        match value
            .namespace
            .ok_or(Error::invalid_format("Namespace not provided"))?
        {
            grpc::resource_namespace::Namespace::Aura(()) => Ok(Self::Aura),
            grpc::resource_namespace::Namespace::UserIcon(()) => Ok(Self::UserIcon),
            grpc::resource_namespace::Namespace::Channel(id) => Ok(Self::Channel(id)),
        }
    }

    fn into_grpc(self) -> Result<Self::Type, Error> {
        Ok(grpc::ResourceNamespace {
            namespace: match self {
                Self::Aura => Some(grpc::resource_namespace::Namespace::Aura(())),
                Self::UserIcon => Some(grpc::resource_namespace::Namespace::UserIcon(())),
                Self::Channel(id) => Some(grpc::resource_namespace::Namespace::Channel(id)),
            },
        })
    }
}

impl From<&ResourceNamespace> for db::ResourceNamespaceType {
    fn from(namespace: &ResourceNamespace) -> Self {
        match namespace {
            ResourceNamespace::Aura => Self::Aura,
            ResourceNamespace::UserIcon => Self::UserIcon,
            ResourceNamespace::Channel(_) => Self::Channel,
        }
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
        let namespace = match value.namespace_type {
            db::ResourceNamespaceType::Aura => ResourceNamespace::Aura,
            db::ResourceNamespaceType::UserIcon => ResourceNamespace::UserIcon,
            db::ResourceNamespaceType::Channel => ResourceNamespace::Channel(value.namespace_id),
        };

        Ok(Self {
            resource_id: ResourceId {
                namespace,
                key: value.key,
            },
            meta: ResourceMeta::from_db(value.meta)?,
            user_id: value.user_id,
        })
    }

    fn into_db(self) -> Result<Self::Type, Error> {
        let namespace_type = db::ResourceNamespaceType::from(&self.resource_id.namespace);

        let namespace_id = match self.resource_id.namespace {
            ResourceNamespace::Aura => String::new(),
            ResourceNamespace::UserIcon => String::new(),
            ResourceNamespace::Channel(id) => id,
        };

        Ok(db::ResourceDescriptor {
            namespace_type,
            namespace_id,
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
    pub name: String,
    pub metadata: FastMap<String, String>,
}

impl JsonDatabaseDomainType for ResourceMeta {}

impl GrpcDomainType for ResourceMeta {
    type Type = grpc::ResourceMeta;

    fn from_grpc(value: Self::Type) -> Result<Self, Error> {
        Ok(Self {
            size: value.size as u32,
            timestamp: Timestamp::from_grpc(
                value
                    .timestamp
                    .ok_or(Error::invalid_format("Timestamp not provided"))?,
            )?,
            name: value.name,
            metadata: value.metadata.into_iter().collect(),
        })
    }

    fn into_grpc(self) -> Result<Self::Type, Error> {
        Ok(Self::Type {
            size: self.size as i32,
            timestamp: Some(self.timestamp.into_grpc()?),
            name: self.name,
            metadata: self.metadata.into_iter().collect(),
        })
    }
}
