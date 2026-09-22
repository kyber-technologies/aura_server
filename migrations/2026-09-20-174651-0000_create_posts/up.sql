CREATE EXTENSION IF NOT EXISTS vector;

CREATE TYPE post_reaction AS ENUM (
    'none',
    'like',
    'dislike'
);

CREATE TABLE posts
(
    post_id   TEXT PRIMARY KEY,
    author_id TEXT        NOT NULL,
    content   JSONB       NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    parent_id TEXT,
    embedding vector(384),

    FOREIGN KEY (author_id)
        REFERENCES users (user_id)
        ON DELETE CASCADE,

    FOREIGN KEY (parent_id)
        REFERENCES posts (post_id)
        ON DELETE CASCADE
);

CREATE INDEX posts_author_id_idx ON posts (author_id);
CREATE INDEX posts_parent_id_idx ON posts (parent_id);
CREATE INDEX posts_timestamp_idx ON posts (timestamp DESC);

CREATE TABLE post_reactions
(
    post_id  TEXT          NOT NULL,
    user_id  TEXT          NOT NULL,
    reaction post_reaction NOT NULL,

    PRIMARY KEY (post_id, user_id),

    FOREIGN KEY (post_id)
        REFERENCES posts (post_id)
        ON DELETE CASCADE,

    FOREIGN KEY (user_id)
        REFERENCES users (user_id)
        ON DELETE CASCADE
);

CREATE INDEX post_reactions_post_id_idx ON post_reactions (post_id);

CREATE INDEX idx_posts_embedding_hnsw
    ON posts USING hnsw (embedding vector_cosine_ops);

CREATE INDEX idx_post_reactions_post_id ON post_reactions (post_id);
