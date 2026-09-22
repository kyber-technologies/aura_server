use crate::config;
use crate::error::Error;
use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use pgvector::Vector;

pub struct TextEmbedder {
    embed: TextEmbedding,
}

impl TextEmbedder {
    pub fn new() -> Result<Self, Error> {
        let config = config::get();

        // TODO: Use Mutex or RwLock to cache instance...?
        let embed = TextEmbedding::try_new(
            TextInitOptions::new(EmbeddingModel::AllMiniLML6V2)
                .with_intra_threads(config.database.embedding_threads)
                .with_max_length(config.database.embedding_max_length),
        )
        .expect("Failed to create embedding");

        Ok(Self { embed })
    }

    pub fn embed(&mut self, texts: &[&str]) -> Result<Vec<Vector>, Error> {
        // TODO: Investigate into batch size.
        Ok(self
            .embed
            .embed(texts, None)
            .map_err(|err| Error::internal(format!("Failed to embed: {err}")))?
            .into_iter()
            .map(Vector::from)
            .collect())
    }
}
