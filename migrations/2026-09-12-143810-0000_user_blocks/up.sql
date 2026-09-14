CREATE TABLE user_blocks
(
    user_id         TEXT NOT NULL,
    blocked_user_id TEXT NOT NULL,

    PRIMARY KEY (user_id, blocked_user_id),

    FOREIGN KEY (user_id)
        REFERENCES users (user_id)
        ON DELETE CASCADE,

    FOREIGN KEY (blocked_user_id)
        REFERENCES users (user_id)
        ON DELETE CASCADE,

    CHECK (user_id <> blocked_user_id)
);

CREATE INDEX user_blocks_blocked_user_id_idx
    ON user_blocks (blocked_user_id);
