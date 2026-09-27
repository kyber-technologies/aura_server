CREATE TABLE user_blocks
(
    user_id         TEXT NOT NULL REFERENCES users (user_id) ON DELETE CASCADE,
    blocked_user_id TEXT NOT NULL REFERENCES users (user_id) ON DELETE CASCADE,

    PRIMARY KEY (user_id, blocked_user_id),
    CONSTRAINT user_blocks_no_self_block CHECK (user_id <> blocked_user_id)
);

CREATE INDEX idx_user_blocks_blocked_user_id ON user_blocks (blocked_user_id);
