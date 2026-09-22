use crate::error::Error;
use std::ops::{Deref, DerefMut};
use tonic::Streaming;
use tonic::codec::CompressionEncoding;
use tonic::codegen::tokio_stream::StreamExt;

/// Maximum size of a message in bytes (4 KiB).
pub const MAX_MESSAGE_SIZE: usize = 1024 * 4;

/// Size of a resource chunk in bytes (2 KiB).
pub const RESOURCE_CHUNK_SIZE: usize = 1024 * 2;

/// The compression encoding to use (Gzip).
// TODO: Investigate into compression
pub const COMPRESSION: CompressionEncoding = CompressionEncoding::Gzip;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn is_valid_ident(i: &str) -> bool {
    i.chars().all(|c| c.is_alphanumeric() || c == '_')
}

pub fn escape_like_pattern(query: &str) -> String {
    let escaped = query
        .replace('\\', r"\\")
        .replace('%', r"\%")
        .replace('_', r"\_");

    format!("%{}%", escaped)
}

pub struct SafeStreaming<T>(Streaming<T>);

impl<T> SafeStreaming<T> {
    pub fn new(stream: Streaming<T>) -> Self {
        Self(stream)
    }

    pub fn into_inner(self) -> Streaming<T> {
        self.0
    }

    pub async fn next_safe(&mut self) -> Option<Result<T, Error>> {
        self.0
            .next()
            .await
            .map(|v| v.map_err(|err| Error::internal(format!("Got invalid stream item: {err}"))))
    }
}

impl<T> Deref for SafeStreaming<T> {
    type Target = Streaming<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for SafeStreaming<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Generated a new unique ID with length 16.
///
/// If 1000 Unique IDs would be generated every second,
/// it would take ~1000 years to have a 1% chance of a collision.
pub fn generate_unique_id() -> String {
    nanoid::format(nanoid::rngs::default, &nanoid::alphabet::SAFE, 16)
}
