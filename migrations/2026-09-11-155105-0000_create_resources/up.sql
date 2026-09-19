CREATE TYPE resource_namespace_type AS ENUM (
    'aura',
    'user_icon',
    'channel'
);

CREATE TABLE resources
(
    namespace_type resource_namespace_type NOT NULL,
    namespace_id   TEXT NOT NULL,
    key            TEXT NOT NULL,
    meta           JSONB NOT NULL,
    user_id        TEXT NOT NULL,

    PRIMARY KEY (namespace_type, namespace_id, key),

    FOREIGN KEY (user_id)
        REFERENCES users (user_id)
        ON DELETE CASCADE
);
