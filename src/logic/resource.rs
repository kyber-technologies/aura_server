use crate::config;
use crate::database::DatabaseConnection;
use crate::database::resource as db;
use crate::database::resource::ResourceNamespaceType;
use crate::error::Error;
use crate::logic::chat;
use crate::schema::resources;
use crate::types::DatabaseDomainType;
use crate::types::resource::{ResourceDescriptor, ResourceId, ResourceNamespace};
use crate::utils::RESOURCE_CHUNK_SIZE;
use diesel::{ExpressionMethods, SelectableHelper};
use diesel::{OptionalExtension, QueryDsl};
use diesel_async::RunQueryDsl;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio_util::io::ReaderStream;
use tonic::codegen::tokio_stream::{Stream, StreamExt};

pub async fn create(
    database: &mut DatabaseConnection,
    desc: ResourceDescriptor,
) -> Result<ResourceDescriptor, Error> {
    if exists(database, &desc.resource_id).await? {
        return Err(Error::already_exists("Resource already exists"));
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
    let namespace_type = ResourceNamespaceType::from(&resource_id.namespace);

    let namespace_id = match &resource_id.namespace {
        ResourceNamespace::Aura => "",
        ResourceNamespace::UserIcon => "",
        ResourceNamespace::Channel(id) => id.as_str(),
    };

    let resource = resources::table
        .filter(resources::namespace_type.eq(namespace_type))
        .filter(resources::namespace_id.eq(namespace_id))
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
    let namespace_type = ResourceNamespaceType::from(&resource_id.namespace);

    let namespace_id = match &resource_id.namespace {
        ResourceNamespace::Aura => "",
        ResourceNamespace::UserIcon => "",
        ResourceNamespace::Channel(id) => id.as_str(),
    };

    Ok(resources::table
        .filter(resources::namespace_type.eq(namespace_type))
        .filter(resources::namespace_id.eq(namespace_id))
        .filter(resources::key.eq(&resource_id.key))
        .select(resources::key)
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
    Ok(match &desc.resource_id.namespace {
        ResourceNamespace::Aura => true,
        ResourceNamespace::UserIcon => true,
        ResourceNamespace::Channel(channel_id) => chat::get_channel(database, channel_id)
            .await?
            .ok_or(Error::not_found("Channel not found"))?
            .members
            .contains_key(user),
    })
}

pub async fn is_upload_authorized(
    database: &mut DatabaseConnection,
    desc: &ResourceDescriptor,
    user_id: &str,
) -> Result<bool, Error> {
    Ok(match &desc.resource_id.namespace {
        ResourceNamespace::Aura => false,
        ResourceNamespace::UserIcon => desc.resource_id.key == user_id,
        ResourceNamespace::Channel(channel_id) => {
            chat::get_channel_member_perm(database, channel_id, user_id)
                .await?
                .is_write_authorized()
        }
    })
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
            Error::internal("Failed to read resource")
        })?;

    let stream = ReaderStream::with_capacity(file, RESOURCE_CHUNK_SIZE).map(|res| {
        res.map_err(|err| {
            tracing::error!("Failed reading file: {err}");
            Error::internal("Failed to read resource")
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
        Error::internal("Failed to write resource")
    })?;

    tokio::pin!(stream);

    let mut buf = BufWriter::with_capacity(RESOURCE_CHUNK_SIZE, file);

    while let Some(data) = stream.next().await {
        buf.write(&data?).await.map_err(|e| {
            tracing::error!("Failed writing file: {e}");
            Error::internal("Failed to write resource")
        })?;
    }

    buf.flush().await.map_err(|e| {
        tracing::error!("Failed flushing file: {e}");
        Error::internal("Failed to write resource")
    })?;

    Ok(())
}

fn build_path(id: &ResourceId) -> PathBuf {
    Path::new(&config::get().service.resource_dir)
        .join(match &id.namespace {
            ResourceNamespace::Aura => "aura".to_string(),
            ResourceNamespace::UserIcon => "user_icon".to_string(),
            ResourceNamespace::Channel(id) => format!("channel.{id}"),
        })
        .join(&id.key)
}
