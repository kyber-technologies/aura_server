CREATE EXTENSION IF NOT EXISTS vector;

CREATE TYPE user_role AS ENUM (
    'user',
    'moderator',
    'admin'
);

CREATE TABLE users
(
    user_id       TEXT PRIMARY KEY,
    username      TEXT        NOT NULL,
    email         TEXT        NOT NULL,
    password      TEXT        NOT NULL,
    role          user_role   NOT NULL,
    icon          JSONB       NOT NULL,
    notifications JSONB       NOT NULL DEFAULT '[]'::jsonb,
    created_at    TIMESTAMPTZ NOT NULL,
    embedding     vector(384),

    CONSTRAINT users_user_id_unique UNIQUE (user_id),
    CONSTRAINT users_email_unique UNIQUE (email)
);

CREATE INDEX idx_users_embedding_hnsw
    ON users USING hnsw (embedding vector_cosine_ops);

CREATE TABLE user_follows
(
    follower_id TEXT        NOT NULL REFERENCES users (user_id) ON DELETE CASCADE,
    followed_id TEXT        NOT NULL REFERENCES users (user_id) ON DELETE CASCADE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    PRIMARY KEY (follower_id, followed_id),
    CONSTRAINT user_follows_no_self_follow CHECK (follower_id <> followed_id)
);

CREATE INDEX idx_user_follows_follower ON user_follows (follower_id);
CREATE INDEX idx_user_follows_followed ON user_follows (followed_id);
