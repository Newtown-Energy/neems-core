CREATE TABLE sessions (
    id TEXT PRIMARY KEY NOT NULL,
    user_id INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    expires_at TIMESTAMP,
    revoked BOOLEAN NOT NULL DEFAULT 0,
    FOREIGN KEY(user_id) REFERENCES users(id)
);
