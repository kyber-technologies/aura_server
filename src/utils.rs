use crate::error::Error;
use aura_rust::Timestamp;
use aura_rust::common::v1::ErrorCode;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{SystemTime, UNIX_EPOCH};
use tonic::Streaming;
use tonic::codec::CompressionEncoding;
use tonic::codegen::tokio_stream::{Stream, StreamExt};

/// Maximum size of a message in bytes (4 KiB).
pub const MAX_MESSAGE_SIZE: usize = 1024 * 4;

/// Size of a resource chunk in bytes (2 KiB).
pub const RESOURCE_CHUNK_SIZE: usize = 1024 * 2;

/// The compression encoding to use (Gzip).
pub const COMPRESSION: CompressionEncoding = CompressionEncoding::Gzip;

pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");

pub fn is_valid_file_name(s: &str) -> bool {
    s.chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '.' || c == '_')
}

pub fn get_timestamp() -> Timestamp {
    Timestamp {
        millis: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap() // TODO: don't panic
            .as_millis() as u64,
    }
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
        let v = self.0.next().await;

        if let Some(v) = v {
            Some(v.map_err(|err| {
                Error::new(
                    ErrorCode::Internal,
                    format!("Got invalid stream item: {err}"),
                )
            }))
        } else {
            None
        }
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

// TODO: use VecDequeue or something else instead
pub struct VecStream<T: Unpin>(Vec<T>);

impl<T: Unpin> VecStream<T> {
    pub fn new(vec: Vec<T>) -> Self {
        Self(vec)
    }

    pub fn once(item: T) -> Self {
        Self::new(vec![item])
    }
}

impl<T: Unpin> Stream for VecStream<T> {
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if this.0.is_empty() {
            return Poll::Ready(None);
        }

        Poll::Ready(Some(this.0.remove(0)))
    }
}
