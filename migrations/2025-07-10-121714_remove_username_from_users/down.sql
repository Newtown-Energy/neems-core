-- Recreate the users table with username column
CREATE TABLE users_old (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    institution_id INTEGER NOT NULL REFERENCES institutions(id),
    totp_secret TEXT NOT NULL
);

-- Copy data back to old structure (username will be NULL, you might want to set a default)
INSERT INTO users_old (id, email, password_hash, created_at, updated_at, institution_id, totp_secret)
SELECT id, email, password_hash, created_at, updated_at, institution_id, totp_secret FROM users;

-- Drop the new table
DROP TABLE users;

-- Rename the old table
ALTER TABLE users_old RENAME TO users;

-- Update usernames (set them to email or some other value)
UPDATE users SET username = email;
