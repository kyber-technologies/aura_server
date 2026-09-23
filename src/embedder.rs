use crate::config;
use crate::error::Error;
use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use pgvector::Vector;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

type Job = (Vec<String>, oneshot::Sender<Result<Vec<Vector>, Error>>);

#[derive(Clone)]
pub struct TextEmbedder {
    sender: mpsc::Sender<Job>,
}

impl TextEmbedder {
    pub fn new() -> Self {
        let config = config::get();

        let (sender, mut receiver) = mpsc::channel::<Job>(100);
        let sender_clone = sender.clone();

        tokio::spawn(async move {
            let semaphore = Arc::new(tokio::sync::Semaphore::new(
                config.database.embedding_pool_size.max(1),
            ));

            while let Some((texts, respond_to)) = receiver.recv().await {
                let permit = match semaphore.clone().acquire_owned().await {
                    Ok(p) => p,
                    Err(_) => break,
                };

                tokio::task::spawn_blocking(move || {
                    let result = Self::generate_embeddings(&texts);
                    let _ = respond_to.send(result);

                    drop(permit);
                });
            }
        });

        Self {
            sender: sender_clone,
        }
    }

    fn generate_embeddings(texts: &[String]) -> Result<Vec<Vector>, Error> {
        let config = config::get();

        let mut model = TextEmbedding::try_new(
            TextInitOptions::new(EmbeddingModel::AllMiniLML6V2)
                .with_intra_threads(config.database.embedding_threads)
                .with_max_length(config.database.embedding_max_length),
        )
        .map_err(|err| Error::internal(format!("Failed to initialize model: {err}")))?;

        let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();

        let embeddings = model
            .embed(&text_refs, Some(32))
            .map_err(|err| Error::internal(format!("Failed to embed text: {err}")))?;

        Ok(embeddings.into_iter().map(Vector::from).collect())
    }

    pub async fn embed(&self, texts: &[&str]) -> Result<Vec<Vector>, Error> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let owned_texts: Vec<String> = texts.iter().map(|&s| s.to_string()).collect();
        let (tx, rx) = oneshot::channel();

        self.sender
            .send((owned_texts, tx))
            .await
            .map_err(|_| Error::internal("Embedding worker channel closed"))?;

        rx.await
            .map_err(|_| Error::internal("Embedding task panicked or dropped"))?
    }
}
