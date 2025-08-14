-- Recreate users table without CASCADE constraint (original version)

DROP TABLE users;
CREATE TABLE users (
    id INTEGER PRIMARY KEY NOT NULL,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    company_id INTEGER NOT NULL REFERENCES companies(id),
    totp_secret TEXT NOT NULL
);
