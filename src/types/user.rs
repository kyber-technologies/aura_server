use crate::database::user as db;
use crate::error::Error;
use crate::types::chat::{Channel, Message};
use crate::types::common::Timestamp;
use crate::types::resource::ResourceId;
use crate::types::{DatabaseDomainType, GrpcDomainType, JsonDatabaseDomainType, UniqueId};
use aura_rust::user::v1 as grpc;
use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq)]
pub struct User {
    pub user_id: String,
    pub username: String,
    pub email: String,
    pub password: String,
    pub role: UserRole,
    pub created_at: Timestamp,
    pub icon: ResourceId,
    pub notifications: Notifications,
    pub settings: UserSettings,
    pub channels: Vec<Channel>,
    pub followers: Vec<String>,
    pub following: Vec<String>,
}

impl User {
    pub fn into_profile(self) -> UserProfile {
        UserProfile {
            user_id: self.user_id,
            username: self.username,
            role: self.role,
            icon: self.icon,
            created_at: self.created_at,
            followers: self.followers.len() as u32,
            following: self.following.len() as u32,
        }
    }
}

impl GrpcDomainType for User {
    type Type = grpc::User;

    fn from_grpc(value: Self::Type) -> Result<Self, Error> {
        let role = value.role();

        Ok(Self {
            user_id: value.user_id,
            username: value.username,
            email: value.email,
            password: value.password,
            role: UserRole::from_grpc(role)?,
            created_at: Timestamp::from_grpc(
                value
                    .created_at
                    .ok_or(Error::invalid_format("Created at not provided"))?,
            )?,
            icon: ResourceId::from_grpc(
                value
                    .icon
                    .ok_or(Error::invalid_format("Icon not provided"))?,
            )?,
            notifications: Notifications::from_grpc(value.notifications)?,
            settings: UserSettings::from_grpc(
                value
                    .settings
                    .ok_or(Error::invalid_format("Settings not provided"))?,
            )?,
            channels: value
                .channels
                .into_iter()
                .map(Channel::from_grpc)
                .collect::<Result<Vec<_>, Error>>()?,
            followers: value.followers,
            following: value.following,
        })
    }

    fn into_grpc(self) -> Result<Self::Type, Error> {
        Ok(grpc::User {
            user_id: self.user_id,
            username: self.username,
            email: self.email,
            password: if cfg!(feature = "testing") {
                // Keep password in testing mode
                self.password
            } else {
                // Return blank password in production
                String::new()
            },
            role: UserRole::into_grpc(self.role)?.into(),
            created_at: Some(self.created_at.into_grpc()?),
            icon: Some(self.icon.into_grpc()?),
            notifications: self.notifications.into_grpc()?,
            settings: Some(self.settings.into_grpc()?),
            channels: self
                .channels
                .into_iter()
                .map(|c| c.into_grpc())
                .collect::<Result<Vec<_>, _>>()?,
            followers: self.followers,
            following: self.following,
        })
    }
}

impl DatabaseDomainType for User {
    type Type = db::UserData;

    fn from_db(value: Self::Type) -> Result<Self, Error> {
        Ok(Self {
            user_id: value.user.user_id,
            username: value.user.username,
            email: value.user.email,
            password: value.user.password,
            role: value.user.role,
            created_at: Timestamp(value.user.created_at),
            icon: ResourceId::from_db(value.user.icon)?,
            notifications: Notifications::from_db(value.user.notifications)?,
            settings: UserSettings::from_db(value.user.settings)?,
            channels: value
                .channels
                .into_iter()
                .map(Channel::from_db)
                .collect::<Result<Vec<_>, _>>()?,
            followers: value.followers,
            following: value.following,
        })
    }

    fn into_db(self) -> Result<Self::Type, Error> {
        Ok(db::UserData {
            user: db::User {
                user_id: self.user_id,
                username: self.username,
                email: self.email,
                password: self.password,
                role: self.role,
                icon: self.icon.into_db()?,
                notifications: self.notifications.into_db()?,
                created_at: self.created_at.0,
                embedding: None,
                settings: self.settings.into_db()?,
            },
            channels: self
                .channels
                .into_iter()
                .map(|c| c.into_db())
                .collect::<Result<Vec<_>, Error>>()?,
            followers: self.followers,
            following: self.following,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct UserProfile {
    pub user_id: String,
    pub username: String,
    pub role: UserRole,
    pub icon: ResourceId,
    pub created_at: Timestamp,
    pub followers: u32,
    pub following: u32,
}

impl GrpcDomainType for UserProfile {
    type Type = grpc::UserProfile;

    fn from_grpc(value: Self::Type) -> Result<Self, Error> {
        let role = value.role();

        Ok(Self {
            user_id: value.user_id,
            username: value.username,
            role: UserRole::from_grpc(role)?,
            icon: ResourceId::from_grpc(
                value
                    .icon
                    .ok_or(Error::invalid_format("Icon not provided"))?,
            )?,
            created_at: Timestamp::from_grpc(
                value
                    .created_at
                    .ok_or(Error::invalid_format("Created at not provided"))?,
            )?,
            followers: value.followers,
            following: value.following,
        })
    }

    fn into_grpc(self) -> Result<Self::Type, Error> {
        Ok(grpc::UserProfile {
            user_id: self.user_id,
            username: self.username,
            role: self.role.into_grpc()? as i32,
            icon: Some(self.icon.into_grpc()?),
            created_at: Some(self.created_at.into_grpc()?),
            followers: self.followers,
            following: self.following,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Notifications(pub Vec<Notification>);

impl GrpcDomainType for Notifications {
    type Type = Vec<grpc::Notification>;

    fn from_grpc(value: Self::Type) -> Result<Self, Error> {
        Ok(Self(
            value
                .into_iter()
                .map(Notification::from_grpc)
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }

    fn into_grpc(self) -> Result<Self::Type, Error> {
        self.0
            .into_iter()
            .map(|n| n.into_grpc())
            .collect::<Result<Vec<_>, _>>()
    }
}

impl JsonDatabaseDomainType for Notifications {}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Notification {
    Invite {
        notification_id: UniqueId,
        timestamp: Timestamp,
        channel_id: UniqueId,
        invited_by: String,
        uninvited: bool,
    },
    Message {
        notification_id: UniqueId,
        timestamp: Timestamp,
        channel_id: UniqueId,
        sender_id: String,
        message: Message,
    },
    Comment {
        notification_id: UniqueId,
        timestamp: Timestamp,
        post_id: UniqueId,
        commend_id: UniqueId,
        sender_id: String,
    },
}

impl Notification {
    pub fn timestamp(&self) -> &Timestamp {
        match self {
            Notification::Invite { timestamp, .. } => timestamp,
            Notification::Message { timestamp, .. } => timestamp,
            Notification::Comment { timestamp, .. } => timestamp,
        }
    }

    pub fn should_notify(&self, settings: &UserSettings) -> bool {
        match self {
            Notification::Invite { .. } => settings.notify_invite,
            Notification::Message { .. } => settings.notify_message,
            Notification::Comment { .. } => settings.notify_comment,
        }
    }
}

impl GrpcDomainType for Notification {
    type Type = grpc::Notification;

    fn from_grpc(value: Self::Type) -> Result<Self, Error> {
        match value
            .notification
            .ok_or(Error::invalid_format("Notification not provided"))?
        {
            grpc::notification::Notification::Invite(not) => Ok(Self::Invite {
                notification_id: value.notification_id as UniqueId,
                timestamp: Timestamp::from_grpc(
                    value
                        .timestamp
                        .ok_or(Error::invalid_format("Timestamp not provided"))?,
                )?,
                channel_id: not.channel_id as UniqueId,
                invited_by: not.invited_by,
                uninvited: not.uninvited,
            }),
            grpc::notification::Notification::Message(not) => Ok(Self::Message {
                notification_id: value.notification_id as UniqueId,
                timestamp: Timestamp::from_grpc(
                    value
                        .timestamp
                        .ok_or(Error::invalid_format("Timestamp not provided"))?,
                )?,
                channel_id: not.channel_id as UniqueId,
                sender_id: not.sender_id,
                message: Message::from_grpc(
                    not.message
                        .ok_or(Error::invalid_format("Message not provided"))?,
                )?,
            }),
            grpc::notification::Notification::Comment(not) => Ok(Self::Comment {
                notification_id: value.notification_id as UniqueId,
                timestamp: Timestamp::from_grpc(
                    value
                        .timestamp
                        .ok_or(Error::invalid_format("Timestamp not provided"))?,
                )?,
                post_id: not.post_id as UniqueId,
                commend_id: not.comment_id as UniqueId,
                sender_id: not.sender_id,
            }),
        }
    }

    fn into_grpc(self) -> Result<Self::Type, Error> {
        match self {
            Notification::Invite {
                notification_id,
                timestamp,
                channel_id,
                invited_by,
                uninvited,
            } => Ok(grpc::Notification {
                notification_id: notification_id as u64,
                timestamp: Some(timestamp.into_grpc()?),
                notification: Some(grpc::notification::Notification::Invite(
                    grpc::InviteNotification {
                        channel_id: channel_id as u64,
                        invited_by,
                        uninvited,
                    },
                )),
            }),
            Notification::Message {
                notification_id,
                timestamp,
                channel_id,
                sender_id,
                message,
            } => Ok(grpc::Notification {
                notification_id: notification_id as u64,
                timestamp: Some(timestamp.into_grpc()?),
                notification: Some(grpc::notification::Notification::Message(
                    grpc::MessageNotification {
                        channel_id: channel_id as u64,
                        sender_id,
                        message: Some(message.into_grpc()?),
                    },
                )),
            }),
            Notification::Comment {
                notification_id,
                timestamp,
                post_id,
                commend_id,
                sender_id,
            } => Ok(grpc::Notification {
                notification_id: notification_id as u64,
                timestamp: Some(timestamp.into_grpc()?),
                notification: Some(grpc::notification::Notification::Comment(
                    grpc::CommentNotification {
                        post_id: post_id as u64,
                        comment_id: commend_id as u64,
                        sender_id,
                    },
                )),
            }),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, DbEnum)]
#[db_enum(existing_type_path = "crate::schema::sql_types::UserRole")]
pub enum UserRole {
    #[db_enum(rename = "user")]
    User,
    #[db_enum(rename = "moderator")]
    Moderator,
    #[db_enum(rename = "admin")]
    Admin,
}

impl GrpcDomainType for UserRole {
    type Type = grpc::UserRole;

    fn from_grpc(value: Self::Type) -> Result<Self, Error> {
        match value {
            grpc::UserRole::UserUnspecified => Ok(UserRole::User),
            grpc::UserRole::Moderator => Ok(UserRole::Moderator),
            grpc::UserRole::Admin => Ok(UserRole::Admin),
        }
    }

    fn into_grpc(self) -> Result<Self::Type, Error> {
        match self {
            UserRole::User => Ok(grpc::UserRole::UserUnspecified),
            UserRole::Moderator => Ok(grpc::UserRole::Moderator),
            UserRole::Admin => Ok(grpc::UserRole::Admin),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct UserSettings {
    pub allow_invites: bool,
    pub algo_like_weight: f32,
    pub algo_dislike_weight: f32,
    pub algo_comment_weight: f32,
    pub algo_time_decay: f32,
    pub notify_invite: bool,
    pub notify_message: bool,
    pub notify_comment: bool,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            allow_invites: true,
            algo_like_weight: 0.55,
            algo_dislike_weight: 0.5,
            algo_comment_weight: 0.8,
            algo_time_decay: 0.4,
            notify_invite: true,
            notify_message: true,
            notify_comment: true,
        }
    }
}

impl GrpcDomainType for UserSettings {
    type Type = grpc::UserSettings;

    fn from_grpc(value: Self::Type) -> Result<Self, Error> {
        Ok(Self {
            allow_invites: value.allow_invites,
            algo_like_weight: value.algo_like_weight,
            algo_dislike_weight: value.algo_dislike_weight,
            algo_comment_weight: value.algo_comment_weight,
            algo_time_decay: value.algo_time_decay,
            notify_invite: value.notify_invite,
            notify_message: value.notify_message,
            notify_comment: value.notify_comment,
        })
    }

    fn into_grpc(self) -> Result<Self::Type, Error> {
        Ok(grpc::UserSettings {
            reset_algo_tags: Vec::new(),
            allow_invites: self.allow_invites,
            algo_like_weight: self.algo_like_weight,
            algo_dislike_weight: self.algo_dislike_weight,
            algo_comment_weight: self.algo_comment_weight,
            algo_time_decay: self.algo_time_decay,
            notify_invite: self.notify_invite,
            notify_message: self.notify_message,
            notify_comment: self.notify_comment,
        })
    }
}

impl JsonDatabaseDomainType for UserSettings {}
