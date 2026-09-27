CREATE TABLE messages
(
    message_id BIGINT PRIMARY KEY,
    channel_id BIGINT      NOT NULL REFERENCES channels (channel_id) ON DELETE CASCADE,
    user_id    TEXT        NOT NULL REFERENCES users (user_id) ON DELETE CASCADE,
    content    JSONB       NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_messages_channel_created ON messages (channel_id, created_at DESC);

CREATE INDEX idx_messages_user_id ON messages (user_id);
