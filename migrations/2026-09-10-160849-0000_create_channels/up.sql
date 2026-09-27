CREATE TYPE channel_permission AS ENUM (
    'read_only',
    'read_write',
    'manager'
);

CREATE TABLE channels
(
    channel_id  BIGINT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT NOT NULL
);

CREATE TABLE channel_members
(
    channel_id BIGINT             NOT NULL REFERENCES channels (channel_id) ON DELETE CASCADE,
    user_id    TEXT               NOT NULL REFERENCES users (user_id) ON DELETE CASCADE,
    permission channel_permission NOT NULL,

    PRIMARY KEY (channel_id, user_id)
);

CREATE INDEX idx_channel_members_user_id ON channel_members (user_id);
