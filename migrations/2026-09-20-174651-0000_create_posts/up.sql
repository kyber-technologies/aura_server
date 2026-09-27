CREATE
EXTENSION IF NOT EXISTS vector;

CREATE TYPE post_reaction AS ENUM (
    'none',
    'like',
    'dislike'
);

CREATE TABLE posts
(
    post_id   BIGINT PRIMARY KEY,
    author_id TEXT        NOT NULL REFERENCES users (user_id) ON DELETE CASCADE,
    content   JSONB       NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL,
    parent_id BIGINT REFERENCES posts (post_id) ON DELETE CASCADE,
    embedding vector(384)
);

CREATE INDEX idx_posts_author_id ON posts (author_id);
CREATE INDEX idx_posts_parent_id ON posts (parent_id) WHERE parent_id IS NOT NULL;
CREATE INDEX idx_posts_timestamp_desc ON posts (timestamp DESC);

CREATE INDEX idx_posts_embedding_hnsw
    ON posts USING hnsw (embedding vector_cosine_ops);

CREATE TABLE post_reactions
(
    post_id  BIGINT        NOT NULL REFERENCES posts (post_id) ON DELETE CASCADE,
    user_id  TEXT          NOT NULL REFERENCES users (user_id) ON DELETE CASCADE,
    reaction post_reaction NOT NULL,

    PRIMARY KEY (post_id, user_id)
);

CREATE INDEX idx_post_reactions_user_id ON post_reactions (user_id);
