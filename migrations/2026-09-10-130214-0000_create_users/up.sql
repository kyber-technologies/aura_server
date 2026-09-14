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

    CONSTRAINT users_user_id_unique UNIQUE (user_id),
    CONSTRAINT users_email_unique UNIQUE (email)
);
