use crate::database::posting as db;
use crate::error::Error;
use crate::types::common::Timestamp;
use crate::types::resource::Content;
use crate::types::{DatabaseDomainType, FastMap, GrpcDomainType};
use aura_rust::posting::v1 as grpc;
use diesel_derive_enum::DbEnum;

#[derive(Clone, Debug)]
pub struct Post {
    pub post_id: String,
    pub author_id: String,
    pub content: Content,
    pub timestamp: Timestamp,
    pub parent: Option<String>,
    pub reactions: FastMap<PostReaction, u32>,
    pub reaction: PostReaction,
}

impl GrpcDomainType for Post {
    type Type = grpc::Post;

    fn from_grpc(value: Self::Type) -> Result<Self, Error> {
        let reaction = value.reaction();

        Ok(Self {
            post_id: value.post_id,
            author_id: value.author_id,
            content: Content::from_grpc(
                value
                    .content
                    .ok_or(Error::invalid_format("Content not provided"))?,
            )?,
            timestamp: Timestamp::from_grpc(
                value
                    .timestamp
                    .ok_or(Error::invalid_format("Timestamp not provided"))?,
            )?,
            parent: value.parent,
            reactions: value
                .reactions
                .into_iter()
                .map(|(r, i)| {
                    Ok((
                        PostReaction::from_grpc(
                            grpc::PostReaction::from_str_name(&r)
                                .ok_or(Error::invalid_format("Invalid post reaction"))?,
                        )?,
                        i,
                    ))
                })
                .collect::<Result<FastMap<PostReaction, u32>, Error>>()?,
            reaction: PostReaction::from_grpc(reaction)?,
        })
    }

    fn into_grpc(self) -> Result<Self::Type, Error> {
        Ok(grpc::Post {
            post_id: self.post_id,
            author_id: self.author_id,
            content: Some(self.content.into_grpc()?),
            timestamp: Some(self.timestamp.into_grpc()?),
            parent: self.parent,
            reactions: self
                .reactions
                .into_iter()
                .map(|(r, i)| Ok((r.into_grpc()?.as_str_name().to_owned(), i)))
                .collect::<Result<std::collections::HashMap<String, u32>, Error>>()?,
            reaction: self.reaction.into_grpc()? as i32,
        })
    }
}

impl DatabaseDomainType for Post {
    type Type = db::PostData;

    fn from_db(value: Self::Type) -> Result<Self, Error> {
        Ok(Self {
            post_id: value.post.post_id,
            author_id: value.post.author_id,
            content: Content::from_db(value.post.content)?,
            timestamp: Timestamp(value.post.timestamp),
            parent: value.post.parent_id,
            reactions: value
                .reaction_counts
                .into_iter()
                .map(|(r, c)| (r, c as u32))
                .collect(),
            reaction: value.user_reaction,
        })
    }

    fn into_db(self) -> Result<Self::Type, Error> {
        Ok(db::PostData {
            post: db::Post {
                post_id: self.post_id,
                author_id: self.author_id,
                content: self.content.into_db()?,
                timestamp: self.timestamp.0,
                parent_id: self.parent,
                embedding: None,
            },
            reaction_counts: self
                .reactions
                .into_iter()
                .map(|(r, c)| (r, c as i64))
                .collect(),
            user_reaction: self.reaction,
        })
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, DbEnum)]
#[db_enum(existing_type_path = "crate::schema::sql_types::PostReaction")]
pub enum PostReaction {
    #[db_enum(rename = "none")]
    None,
    #[db_enum(rename = "like")]
    Like,
    #[db_enum(rename = "dislike")]
    Dislike,
}

impl GrpcDomainType for PostReaction {
    type Type = grpc::PostReaction;

    fn from_grpc(value: Self::Type) -> Result<Self, Error> {
        match value {
            grpc::PostReaction::Unspecified => Ok(Self::None),
            grpc::PostReaction::Like => Ok(Self::Like),
            grpc::PostReaction::Dislike => Ok(Self::Dislike),
        }
    }

    fn into_grpc(self) -> Result<Self::Type, Error> {
        match self {
            Self::None => Ok(grpc::PostReaction::Unspecified),
            Self::Like => Ok(grpc::PostReaction::Like),
            Self::Dislike => Ok(grpc::PostReaction::Dislike),
        }
    }
}
