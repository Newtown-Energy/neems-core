-- Rollback: Make totp_secret column required again in users table
-- This will fail if there are any NULL values in totp_secret

-- Drop triggers that depend on users table
DROP TRIGGER IF EXISTS enforce_newtown_roles_restriction;
DROP TRIGGER IF EXISTS enforce_newtown_roles_restriction_update;

-- Drop the view that depends on users table
DROP VIEW IF EXISTS users_without_roles;

-- Create new users table with required totp_secret
CREATE TABLE users_new (
    id INTEGER PRIMARY KEY NOT NULL,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    company_id INTEGER NOT NULL REFERENCES companies(id),
    totp_secret TEXT NOT NULL
);

-- Copy data from old table to new table (this will fail if any totp_secret is NULL)
INSERT INTO users_new (id, email, password_hash, created_at, updated_at, company_id, totp_secret)
SELECT id, email, password_hash, created_at, updated_at, company_id, totp_secret
FROM users;

-- Drop the old table
DROP TABLE users;

-- Rename the new table
ALTER TABLE users_new RENAME TO users;

-- Recreate the view
CREATE VIEW users_without_roles AS
SELECT u.id, u.email, u.company_id
FROM users u
LEFT JOIN user_roles ur ON u.id = ur.user_id
WHERE ur.user_id IS NULL;

-- Recreate the triggers
CREATE TRIGGER enforce_newtown_roles_restriction
BEFORE INSERT ON user_roles
FOR EACH ROW
BEGIN
    -- Check if the role being assigned is a Newtown-specific role
    -- and if the user is not from Newtown Energy company
    SELECT CASE
        WHEN NEW.role_id IN (
            SELECT id FROM roles WHERE name IN ('newtown-admin', 'newtown-staff')
        ) AND NEW.user_id NOT IN (
            SELECT u.id FROM users u 
            JOIN companies i ON u.company_id = i.id 
            WHERE i.name = 'Newtown Energy'
        )
        THEN RAISE(ABORT, 'Newtown roles (newtown-admin, newtown-staff) can only be assigned to Newtown Energy company users')
    END;
END;

CREATE TRIGGER enforce_newtown_roles_restriction_update
BEFORE UPDATE ON user_roles
FOR EACH ROW
BEGIN
    -- Check if the role being assigned is a Newtown-specific role
    -- and if the user is not from Newtown Energy company
    SELECT CASE
        WHEN NEW.role_id IN (
            SELECT id FROM roles WHERE name IN ('newtown-admin', 'newtown-staff')
        ) AND NEW.user_id NOT IN (
            SELECT u.id FROM users u 
            JOIN companies i ON u.company_id = i.id 
            WHERE i.name = 'Newtown Energy'
        )
        THEN RAISE(ABORT, 'Newtown roles (newtown-admin, newtown-staff) can only be assigned to Newtown Energy company users')
    END;
END;