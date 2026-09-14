CREATE TABLE messages
(
    message_id TEXT PRIMARY KEY,
    channel_id TEXT        NOT NULL,
    user_id    TEXT        NOT NULL,
    content    JSONB       NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,

    FOREIGN KEY (channel_id)
        REFERENCES channels (channel_id)
        ON DELETE CASCADE,

    FOREIGN KEY (user_id)
        REFERENCES users (user_id)
        ON DELETE CASCADE
);

CREATE INDEX messages_channel_id_idx
    ON messages (channel_id);
