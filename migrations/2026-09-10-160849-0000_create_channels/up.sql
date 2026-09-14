CREATE TYPE channel_permission AS ENUM (
    'read_only',
    'read_write',
    'manager'
);

CREATE TABLE channels
(
    channel_id  TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT NOT NULL
);

CREATE TABLE channel_members
(
    channel_id TEXT               NOT NULL,
    user_id    TEXT               NOT NULL,
    permission channel_permission NOT NULL,

    PRIMARY KEY (channel_id, user_id),

    FOREIGN KEY (channel_id)
        REFERENCES channels (channel_id)
        ON DELETE CASCADE,

    FOREIGN KEY (user_id)
        REFERENCES users (user_id)
        ON DELETE CASCADE
);
