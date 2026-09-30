CREATE TYPE resource_namespace_type AS ENUM (
    'aura',
    'user_icon',
    'channel',
    'post'
);

CREATE TABLE resources
(
    namespace_type resource_namespace_type NOT NULL,
    namespace_id   TEXT                    NOT NULL,
    key            TEXT                    NOT NULL,
    meta           JSONB                   NOT NULL,
    user_id        TEXT                    NOT NULL REFERENCES users (user_id) ON DELETE CASCADE,

    PRIMARY KEY (namespace_type, namespace_id, key)
);

CREATE INDEX idx_resources_user_id ON resources (user_id);
