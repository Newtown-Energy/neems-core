-- Drop the deleted tables
DROP TABLE IF EXISTS deleted_users;
DROP TABLE IF EXISTS deleted_companies;

-- Recreate sessions table without CASCADE constraint (original version)
DROP TABLE sessions;
CREATE TABLE sessions (
    id TEXT PRIMARY KEY NOT NULL,
    user_id INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    expires_at TIMESTAMP,
    revoked BOOLEAN NOT NULL DEFAULT 0,
    FOREIGN KEY(user_id) REFERENCES users(id)
);
