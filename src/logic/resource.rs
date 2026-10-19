use crate::database::resource as db;
use crate::database::resource::ResourceNamespaceType;
use crate::error::Error;
use crate::logic::{chat, posting};
use crate::schema::resources;
use crate::state::database::DatabaseConnection;
use crate::types::DatabaseDomainType;
use crate::types::resource::{ResourceDescriptor, ResourceId, ResourceNamespace};
use crate::{config, utils};
use diesel::pg::Pg;
use diesel::{BoolExpressionMethods, BoxableExpression, ExpressionMethods, SelectableHelper};
use diesel::{OptionalExtension, QueryDsl};
use diesel_async::RunQueryDsl;
use image::imageops::FilterType;
use smol_str::format_smolstr;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::process::Command;
use tokio_util::io::ReaderStream;
use tonic::codegen::tokio_stream::{Stream, StreamExt};

pub async fn create(
    database: &mut DatabaseConnection,
    desc: ResourceDescriptor,
) -> Result<ResourceDescriptor, Error> {
    let max_upload_bytes = config::get().service.max_upload_size;

    if desc.meta.size > max_upload_bytes {
        return Err(Error::invalid_format(format_smolstr!(
            "Resource size ({} bytes) exceeds maximum allowed upload limit ({} bytes)",
            desc.meta.size,
            max_upload_bytes
        )));
    }

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
    resource_ids: &[ResourceId],
) -> Result<Vec<ResourceDescriptor>, Error> {
    utils::validate_item_length(resource_ids.len() as u32)?;

    if resource_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut query = resources::table.into_boxed();

    let mut condition: Option<
        Box<dyn BoxableExpression<resources::table, Pg, SqlType = diesel::sql_types::Bool>>,
    > = None;

    for resource_id in resource_ids {
        let namespace_type = ResourceNamespaceType::from(&resource_id.namespace);
        let namespace_id = resource_id.namespace.namespace_id();

        let clause = resources::namespace_type
            .eq(namespace_type)
            .and(resources::namespace_id.eq(namespace_id.to_string()))
            .and(resources::key.eq(&resource_id.key));

        condition = match condition {
            Some(acc) => Some(Box::new(acc.or(clause))),
            None => Some(Box::new(clause)),
        };
    }

    if let Some(cond) = condition {
        query = query.filter(cond);
    }

    let db_resources = query
        .select(db::ResourceDescriptor::as_select())
        .load::<db::ResourceDescriptor>(database)
        .await?;

    db_resources
        .into_iter()
        .map(ResourceDescriptor::from_db)
        .collect::<Result<Vec<_>, _>>()
}

pub async fn exists(
    database: &mut DatabaseConnection,
    resource_id: &ResourceId,
) -> Result<bool, Error> {
    let namespace_type = ResourceNamespaceType::from(&resource_id.namespace);

    let namespace_id = resource_id.namespace.namespace_id();

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
        ResourceNamespace::Channel(channel_id) => chat::get_channel(
            database,
            channel_id.parse().map_err(|err| {
                Error::internal(format_smolstr!(
                    "Failed to parse channel ID from resource namespace: {err}"
                ))
            })?,
        )
        .await?
        .ok_or(Error::not_found("Channel not found"))?
        .members
        .contains_key(user),
        ResourceNamespace::Post(_) => true,
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

        ResourceNamespace::Channel(channel_id) => chat::get_channel_member_perm(
            database,
            channel_id.parse().map_err(|err| {
                Error::internal(format_smolstr!(
                    "Failed to parse channel ID from resource namespace: {err}"
                ))
            })?,
            user_id,
        )
        .await?
        .is_write_authorized(),

        ResourceNamespace::Post(post_id) => {
            let post_id_parsed = post_id.parse().map_err(|err| {
                Error::internal(format_smolstr!(
                    "Failed to parse post ID from resource namespace: {err}"
                ))
            })?;

            match posting::get_author(database, post_id_parsed).await? {
                Some(author_id) => author_id == user_id,
                None => false,
            }
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

    let stream =
        ReaderStream::with_capacity(file, config::get().service.resource_chunk_size).map(|res| {
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

    let file = fs::File::create(&path).await.map_err(|e| {
        tracing::error!("Failed opening write file: {e}");
        Error::internal("Failed to write resource")
    })?;

    tokio::pin!(stream);

    let max_upload_bytes = config::get().service.max_upload_size as u64;
    let mut total_written: u64 = 0;

    let mut buf = BufWriter::with_capacity(config::get().service.resource_chunk_size, file);

    while let Some(data_res) = stream.next().await {
        let data = data_res?;
        let chunk_len = data.len() as u64;

        if total_written + chunk_len > max_upload_bytes {
            drop(buf);
            let _ = fs::remove_file(&path).await;

            tracing::warn!(
                "Aborted writing resource {:?}: payload exceeds maximum allowed upload size ({} bytes)",
                id,
                max_upload_bytes
            );
            return Err(Error::invalid_format(
                "Uploaded stream exceeded maximum allowed file size",
            ));
        }

        buf.write_all(&data).await.map_err(|e| {
            tracing::error!("Failed writing file: {e}");
            Error::internal("Failed to write resource")
        })?;

        total_written += chunk_len;
    }

    buf.flush().await.map_err(|e| {
        tracing::error!("Failed flushing file: {e}");
        Error::internal("Failed to write resource")
    })?;

    Ok(())
}

pub async fn write_stream_to_path(
    path: &Path,
    mut stream: impl Stream<Item = Result<Vec<u8>, Error>> + Unpin,
) -> Result<(), Error> {
    let file = fs::File::create(path)
        .await
        .map_err(|e| Error::internal(format_smolstr!("Failed opening write file: {e}")))?;

    let max_upload_bytes = config::get().service.max_upload_size as u64;
    let mut total_written: u64 = 0;
    let mut buf = BufWriter::with_capacity(config::get().service.resource_chunk_size, file);

    while let Some(data_res) = stream.next().await {
        let data = data_res?;
        let chunk_len = data.len() as u64;

        if total_written + chunk_len > max_upload_bytes {
            drop(buf);
            let _ = fs::remove_file(path).await;
            return Err(Error::invalid_format(
                "Uploaded stream exceeded maximum allowed file size",
            ));
        }

        buf.write_all(&data)
            .await
            .map_err(|e| Error::internal(format_smolstr!("Failed writing file: {e}")))?;
        total_written += chunk_len;
    }

    buf.flush()
        .await
        .map_err(|e| Error::internal(format_smolstr!("Failed flushing file: {e}")))?;

    Ok(())
}

pub fn build_path(id: &ResourceId) -> PathBuf {
    Path::new(&config::get().service.resource_dir)
        .join(match &id.namespace {
            ResourceNamespace::Aura => "aura".to_string(),
            ResourceNamespace::UserIcon => "user_icon".to_string(),
            ResourceNamespace::Channel(id) => format!("channel.{id}"),
            ResourceNamespace::Post(id) => format!("post.{id}"),
        })
        .join(&id.key)
}

pub enum ResourceFormat {
    Image,
    Video,
    Audio,
    Other,
}

impl ResourceFormat {
    const ALL: [ResourceFormat; 4] = [
        ResourceFormat::Image,
        ResourceFormat::Video,
        ResourceFormat::Audio,
        ResourceFormat::Other,
    ];

    pub fn from_supported(s: &str) -> Option<Self> {
        match s {
            "png" | "jpg" | "jpeg" | "webp" | "gif" => Some(ResourceFormat::Image),
            "mp4" | "mov" | "mkv" | "webm" => Some(ResourceFormat::Video),
            "flac" | "ogg" | "wav" | "mp3" | "m4a" => Some(ResourceFormat::Audio),
            "pdf" => Some(ResourceFormat::Other),
            _ => None,
        }
    }

    pub fn supported(&self) -> Vec<&str> {
        match self {
            ResourceFormat::Image => vec!["png", "jpg", "jpeg", "webp", "gif"],
            ResourceFormat::Video => vec!["mp4", "mov", "mkv", "webm"],
            ResourceFormat::Audio => vec!["flac", "ogg", "wav", "mp3", "m4a"],
            ResourceFormat::Other => vec!["pdf"],
        }
    }

    pub fn all_supported() -> Vec<String> {
        ResourceFormat::ALL
            .iter()
            .flat_map(|f| f.supported())
            .map(str::to_string)
            .collect()
    }

    pub fn to_format(&self) -> Option<String> {
        match self {
            ResourceFormat::Image => Some("webp".to_string()),
            ResourceFormat::Video => Some("mp4".to_string()),
            ResourceFormat::Audio => Some("ogg".to_string()),
            ResourceFormat::Other => None,
        }
    }

    pub async fn encode(
        &self,
        input_path: &Path,
        output_path: &Path,
        format: &str,
    ) -> Result<(), Error> {
        match format {
            "png" | "jpg" | "jpeg" | "webp" | "gif" => {
                let config = &config::get().service;
                let webp_config = config::get_webp();
                let input_p = input_path.to_path_buf();
                let output_p = output_path.to_path_buf();

                // Parse the format explicitly
                let img_format = match format {
                    "png" => image::ImageFormat::Png,
                    "jpg" | "jpeg" => image::ImageFormat::Jpeg,
                    "webp" => image::ImageFormat::WebP,
                    "gif" => image::ImageFormat::Gif,
                    _ => return Err(Error::invalid_format("Unsupported image format")),
                };

                tokio::task::spawn_blocking(move || {
                    let file = std::fs::File::open(&input_p).map_err(|err| {
                        Error::internal(format_smolstr!("Failed to open temp file path: {err}"))
                    })?;

                    let reader = std::io::BufReader::new(file);

                    let img = image::ImageReader::with_format(reader, img_format)
                        .decode()
                        .map_err(|err| {
                            Error::internal(format_smolstr!("Failed to decode image: {err}"))
                        })?;

                    let resized = if img.width() > config.max_image_width
                        || img.height() > config.max_image_height
                    {
                        img.resize(
                            config.max_image_width,
                            config.max_image_height,
                            FilterType::Lanczos3,
                        )
                    } else {
                        img
                    };

                    let webp = webp::Encoder::from_image(&resized)
                        .map_err(|err| {
                            Error::internal(format_smolstr!(
                                "Failed to encode image into WebP: {err}"
                            ))
                        })?
                        .encode_advanced(webp_config)
                        .map_err(|err| {
                            Error::internal(format_smolstr!(
                                "Failed to encode image into WebP: {err:?}"
                            ))
                        })?;

                    std::fs::write(&output_p, webp.deref()).map_err(|err| {
                        Error::internal(format_smolstr!(
                            "Failed to write encoded image to disk: {err}"
                        ))
                    })?;

                    Ok::<(), Error>(())
                })
                .await
                .map_err(|err| Error::internal(format_smolstr!("Tokio join error: {err:?}")))??;

                Ok(())
            }

            "mp4" | "mov" | "mkv" | "webm" => {
                let args = vec![
                    "-y",
                    "-c:v",
                    "libx264",
                    "-preset",
                    "medium",
                    "-crf",
                    "23",
                    "-c:a",
                    "aac",
                    "-b:a",
                    "128k",
                    "-movflags",
                    "+faststart",
                ];
                run_ffmpeg_file(input_path, output_path, &args).await
            }

            "flac" | "ogg" | "wav" | "mp3" | "m4a" => {
                let args = vec!["-y", "-c:a", "libopus", "-b:a", "96k"];
                run_ffmpeg_file(input_path, output_path, &args).await
            }

            "pdf" => {
                tokio::fs::copy(input_path, output_path)
                    .await
                    .map_err(|err| {
                        Error::internal(format_smolstr!("Failed to copy file: {err}"))
                    })?;
                Ok(())
            }

            _ => Err(Error::invalid_format("Unsupported format")),
        }
    }
}

async fn run_ffmpeg_file(
    input_path: &Path,
    output_path: &Path,
    ffmpeg_args: &[&str],
) -> Result<(), Error> {
    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-i").arg(input_path);
    for arg in ffmpeg_args {
        cmd.arg(arg);
    }
    cmd.arg(output_path);

    let status = cmd.status().await.map_err(|err| {
        Error::internal(format_smolstr!("FFmpeg execution failed to start: {err:?}"))
    })?;

    if !status.success() {
        return Err(Error::internal(format_smolstr!(
            "FFmpeg transcoding failed with exit code: {:?}",
            status.code()
        )));
    }

    Ok(())
}
