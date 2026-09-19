use crate::database::channel as ch_db;
use crate::database::message as msg_db;
use crate::error::Error;
use crate::types::common::Timestamp;
use crate::types::resource::ResourceId;
use crate::types::{DatabaseDomainType, FastMap, GrpcDomainType, JsonDatabaseDomainType};
use aura_rust::chat::v1 as grpc;
use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub struct Channel {
    pub channel_id: String,
    pub name: String,
    pub description: String,
    pub members: FastMap<String, ChannelPermission>,
}

impl GrpcDomainType for Channel {
    type Type = grpc::Channel;

    fn from_grpc(value: Self::Type) -> Result<Self, Error> {
        Ok(Self {
            channel_id: value.channel_id,
            name: value.name,
            description: value.description,
            members: value
                .members
                .into_iter()
                .map(|(k, v)| {
                    Ok((
                        k,
                        ChannelPermission::from_grpc(
                            grpc::ChannelPermission::try_from(v)
                                .map_err(|_| Error::invalid_format())?,
                        )?,
                    ))
                })
                .collect::<Result<FastMap<_, _>, Error>>()?,
        })
    }

    fn into_grpc(self) -> Result<Self::Type, Error> {
        Ok(grpc::Channel {
            channel_id: self.channel_id,
            name: self.name,
            description: self.description,
            members: self
                .members
                .into_iter()
                .map(|(k, v)| Ok((k, v.into_grpc()?.into())))
                .collect::<Result<HashMap<_, _>, Error>>()?,
        })
    }
}

impl DatabaseDomainType for Channel {
    type Type = ch_db::ChannelData;

    fn from_db(value: Self::Type) -> Result<Self, Error> {
        Ok(Self {
            channel_id: value.channel.channel_id,
            name: value.channel.name,
            description: value.channel.description,
            members: value
                .members
                .into_iter()
                .map(|v| (v.user_id, v.permission))
                .collect(),
        })
    }

    fn into_db(self) -> Result<Self::Type, Error> {
        let members = self
            .members
            .into_iter()
            .map(|(k, v)| ch_db::ChannelMember {
                channel_id: self.channel_id.clone(),
                user_id: k,
                permission: v,
            })
            .collect();

        Ok(ch_db::ChannelData {
            channel: ch_db::Channel {
                channel_id: self.channel_id,
                name: self.name,
                description: self.description,
            },
            members,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub message_id: String,
    pub channel_id: String,
    pub user_id: String,
    pub content: Content,
    pub created_at: Timestamp,
}

impl GrpcDomainType for Message {
    type Type = grpc::Message;

    fn from_grpc(value: Self::Type) -> Result<Self, Error> {
        Ok(Self {
            message_id: value.message_id,
            channel_id: value.channel_id,
            user_id: value.user_id,
            content: Content::from_grpc(value.content.ok_or(Error::invalid_format())?)?,
            created_at: Timestamp::from_grpc(value.created_at.ok_or(Error::invalid_format())?)?,
        })
    }

    fn into_grpc(self) -> Result<Self::Type, Error> {
        Ok(grpc::Message {
            message_id: self.message_id,
            channel_id: self.channel_id,
            user_id: self.user_id,
            content: Some(self.content.into_grpc()?),
            created_at: Some(self.created_at.into_grpc()?),
        })
    }
}

impl DatabaseDomainType for Message {
    type Type = msg_db::Message;

    fn from_db(value: Self::Type) -> Result<Self, Error> {
        Ok(Self {
            message_id: value.message_id,
            channel_id: value.channel_id,
            user_id: value.user_id,
            content: Content::from_db(value.content)?,
            created_at: Timestamp(value.created_at),
        })
    }

    fn into_db(self) -> Result<Self::Type, Error> {
        Ok(msg_db::Message {
            message_id: self.message_id,
            channel_id: self.channel_id,
            user_id: self.user_id,
            content: self.content.into_db()?,
            created_at: self.created_at.0,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Content {
    Text(String),
    Resource(ResourceId),
}

impl GrpcDomainType for Content {
    type Type = grpc::Content;

    fn from_grpc(value: Self::Type) -> Result<Self, Error> {
        Ok(match value.content.ok_or(Error::invalid_format())? {
            grpc::content::Content::Text(text) => Content::Text(text),
            grpc::content::Content::Resource(resource) => {
                Content::Resource(ResourceId::from_grpc(resource)?)
            }
        })
    }

    fn into_grpc(self) -> Result<Self::Type, Error> {
        Ok(match self {
            Content::Text(text) => grpc::Content {
                content: Some(grpc::content::Content::Text(text)),
            },
            Content::Resource(resource) => grpc::Content {
                content: Some(grpc::content::Content::Resource(resource.into_grpc()?)),
            },
        })
    }
}

impl JsonDatabaseDomainType for Content {}

#[derive(Debug, Copy, Clone, PartialEq, Eq, DbEnum)]
#[db_enum(existing_type_path = "crate::schema::sql_types::ChannelPermission")]
pub enum ChannelPermission {
    #[db_enum(rename = "read_only")]
    ReadOnly,
    #[db_enum(rename = "read_write")]
    ReadWrite,
    #[db_enum(rename = "manager")]
    Manager,
}

impl ChannelPermission {
    pub fn is_write_authorized(&self) -> bool {
        matches!(
            self,
            ChannelPermission::ReadWrite | ChannelPermission::Manager
        )
    }
}

impl GrpcDomainType for ChannelPermission {
    type Type = grpc::ChannelPermission;

    fn from_grpc(value: Self::Type) -> Result<Self, Error> {
        Ok(match value {
            grpc::ChannelPermission::ReadOnlyUnspecified => ChannelPermission::ReadOnly,
            grpc::ChannelPermission::ReadWrite => ChannelPermission::ReadWrite,
            grpc::ChannelPermission::Manager => ChannelPermission::Manager,
        })
    }

    fn into_grpc(self) -> Result<Self::Type, Error> {
        Ok(match self {
            ChannelPermission::ReadOnly => grpc::ChannelPermission::ReadOnlyUnspecified,
            ChannelPermission::ReadWrite => grpc::ChannelPermission::ReadWrite,
            ChannelPermission::Manager => grpc::ChannelPermission::Manager,
        })
    }
}
