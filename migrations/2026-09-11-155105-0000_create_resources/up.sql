CREATE TABLE resources
(
    namespace TEXT  NOT NULL,
    key       TEXT  NOT NULL,
    meta      JSONB NOT NULL,
    user_id   TEXT  NOT NULL,

    PRIMARY KEY (namespace, key),

    FOREIGN KEY (user_id)
        REFERENCES users (user_id)
        ON DELETE CASCADE
);
