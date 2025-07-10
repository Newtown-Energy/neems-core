-- Remove the username column from users table
CREATE TABLE users_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    institution_id INTEGER NOT NULL REFERENCES institutions(id),
    totp_secret TEXT NOT NULL
);

-- Copy data from old table to new table
INSERT INTO users_new (id, email, password_hash, created_at, updated_at, institution_id, totp_secret)
SELECT id, email, password_hash, created_at, updated_at, institution_id, totp_secret FROM users;

-- Drop the old table
DROP TABLE users;

-- Rename the new table
ALTER TABLE users_new RENAME TO users;
