use crate::database::DatabaseConnection;
use crate::database::resource as db;
use crate::error::Error;
use crate::logic::{chat, user};
use crate::schema::resources;
use crate::types::DatabaseDomainType;
use crate::types::chat::ChannelPermission;
use crate::types::resource::{ResourceDescriptor, ResourceId};
use crate::utils::RESOURCE_CHUNK_SIZE;
use crate::{config, utils};
use aura_rust::common::v1::ErrorCode;
use diesel::{ExpressionMethods, SelectableHelper};
use diesel::{OptionalExtension, QueryDsl};
use diesel_async::RunQueryDsl;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio_util::io::ReaderStream;
use tonic::codegen::tokio_stream::{Stream, StreamExt};

/// Built-in namespace.
pub const BUILTIN_NAMESPACE: &str = "aura";

/// Built-in user icon key.
pub const DEFAULT_ICON_KEY: &str = "default_icon.png";

pub async fn create(
    database: &mut DatabaseConnection,
    desc: ResourceDescriptor,
) -> Result<ResourceDescriptor, Error> {
    if exists(database, &desc.resource_id).await? {
        return Err(Error::new(
            ErrorCode::AlreadyExists,
            "Resource already exists",
        ));
    }

    if !utils::is_valid_file_name(&desc.resource_id.namespace)
        || !utils::is_valid_file_name(&desc.resource_id.key)
    {
        return Err(Error::new(
            ErrorCode::InvalidFormat,
            "Resource ID can only contain alphanumeric and '-', '_', '.' characters",
        ));
    }

    let data = desc.clone().into_db()?;

    diesel::insert_into(resources::table)
        .values(&data)
        .execute(database)
        .await?;

    Ok(desc)
}

pub async fn get(
    database: &mut DatabaseConnection,
    resource_id: &ResourceId,
) -> Result<Option<ResourceDescriptor>, Error> {
    let resource = resources::table
        .filter(resources::namespace.eq(&resource_id.namespace))
        .filter(resources::key.eq(&resource_id.key))
        .select(db::ResourceDescriptor::as_select())
        .first::<db::ResourceDescriptor>(database)
        .await
        .optional()?;

    resource.map(ResourceDescriptor::from_db).transpose()
}

pub async fn exists(
    database: &mut DatabaseConnection,
    resource_id: &ResourceId,
) -> Result<bool, Error> {
    Ok(resources::table
        .filter(resources::namespace.eq(&resource_id.namespace))
        .filter(resources::key.eq(&resource_id.key))
        .select(resources::namespace)
        .first::<String>(database)
        .await
        .optional()?
        .is_some())
}

pub async fn is_download_authorized(
    database: &mut DatabaseConnection,
    desc: &ResourceDescriptor,
    user: &str,
) -> Result<bool, Error> {
    let mut authorized = false;
    let user = user::get(database, user)
        .await?
        .ok_or(Error::new(ErrorCode::NotFound, "User not found"))?;

    if from_builtin(&desc.resource_id).is_some() {
        authorized = true;
    } else if let Some(channel) = chat::get_channel(database, &desc.resource_id.namespace).await?
        && channel.members.contains_key(&user.user_id)
    {
        authorized = true;
    } else if is_user_avatar(&desc.resource_id, None) {
        authorized = true;
    }

    Ok(authorized)
}

pub async fn is_upload_authorized(
    database: &mut DatabaseConnection,
    desc: &ResourceDescriptor,
    user: &str,
) -> Result<bool, Error> {
    let mut authorized = false;
    let user = user::get(database, user)
        .await?
        .ok_or(Error::new(ErrorCode::NotFound, "User not found"))?;

    if let Some(channel) = chat::get_channel(database, &desc.resource_id.namespace).await? {
        let perm =
            chat::get_channel_member_perm(database, &channel.channel_id, &user.user_id).await?;

        authorized = perm == ChannelPermission::Manager || perm == ChannelPermission::ReadWrite;
    } else if is_user_avatar(&desc.resource_id, Some(&user.user_id))
        && desc.resource_id.key.ends_with(".png")
    {
        authorized = true;
    }

    Ok(authorized)
}

pub async fn read(id: ResourceId) -> Result<impl Stream<Item = Result<Vec<u8>, Error>>, Error> {
    let path = build_path(&id);

    let file = fs::OpenOptions::new()
        .read(true)
        .write(false)
        .create(false)
        .append(false)
        .open(&path)
        .await
        .map_err(|e| {
            tracing::error!("Failed opening read file '{path:?}': {e}");
            Error::new(ErrorCode::Internal, "Failed to read resource")
        })?;

    let stream = ReaderStream::with_capacity(file, RESOURCE_CHUNK_SIZE).map(|res| {
        res.map_err(|err| {
            tracing::error!("Failed reading file: {err}");
            Error::new(ErrorCode::Internal, "Failed to read resource")
        })
        .map(|by| by.to_vec())
    });

    Ok(stream)
}

pub async fn write(
    id: ResourceId,
    stream: impl Stream<Item = Result<Vec<u8>, Error>>,
) -> Result<(), Error> {
    let path = build_path(&id);

    fs::create_dir_all(path.parent().expect("Failed to get parent directory"))
        .await
        .expect("Failed to create directory");

    let file = fs::File::create(path).await.map_err(|e| {
        tracing::error!("Failed opening write file: {e}");
        Error::new(ErrorCode::Internal, "Failed to write resource")
    })?;

    tokio::pin!(stream);

    let mut buf = BufWriter::with_capacity(RESOURCE_CHUNK_SIZE, file);

    while let Some(data) = stream.next().await {
        buf.write(&data?).await.map_err(|e| {
            tracing::error!("Failed writing file: {e}");
            Error::new(ErrorCode::Internal, "Failed to write resource")
        })?;
    }

    buf.flush().await.map_err(|e| {
        tracing::error!("Failed flushing file: {e}");
        Error::new(ErrorCode::Internal, "Failed to write resource")
    })?;

    Ok(())
}

pub fn from_builtin(id: &ResourceId) -> Option<PathBuf> {
    if id.namespace.as_str() != BUILTIN_NAMESPACE {
        return None;
    }

    match id.key.as_str() {
        DEFAULT_ICON_KEY => Some(PathBuf::from("aura/default_icon.png")),
        _ => None,
    }
}

pub fn is_user_avatar(id: &ResourceId, user: Option<&str>) -> bool {
    if let Some(user) = user {
        id.namespace.as_str() == format!("user.{user}") && id.key.as_str() == "avatar.png"
    } else {
        id.namespace.as_str().starts_with("user.") && id.key.as_str() == "avatar.png"
    }
}

pub fn build_user_avatar_id(user: &str) -> ResourceId {
    ResourceId {
        namespace: format!("user.{user}"),
        key: "avatar.png".to_string(),
    }
}

fn build_path(id: &ResourceId) -> PathBuf {
    Path::new(&config::get().service.resource_dir)
        .join(&id.namespace)
        .join(&id.key)
}
