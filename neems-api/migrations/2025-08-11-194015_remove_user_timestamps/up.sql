-- Remove timestamp columns from users table
-- Need to handle all foreign keys, triggers, and views carefully

-- Temporarily drop view that references users table
DROP VIEW IF EXISTS users_without_roles;

-- Temporarily drop ALL triggers that might reference users
DROP TRIGGER IF EXISTS users_delete_log;
DROP TRIGGER IF EXISTS users_update_log; 
DROP TRIGGER IF EXISTS users_insert_log;
DROP TRIGGER IF EXISTS enforce_newtown_roles_restriction;
DROP TRIGGER IF EXISTS enforce_newtown_roles_restriction_update;
DROP TRIGGER IF EXISTS prevent_user_without_roles;
DROP TRIGGER IF EXISTS sessions_insert_log;
DROP TRIGGER IF EXISTS sessions_update_log;
DROP TRIGGER IF EXISTS sessions_delete_log;

-- Disable foreign key constraints temporarily
PRAGMA foreign_keys = OFF;

-- Create new users table without timestamp columns
CREATE TABLE users_new (
    id INTEGER PRIMARY KEY NOT NULL,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    company_id INTEGER NOT NULL REFERENCES companies(id),
    totp_secret TEXT
);

-- Copy existing data (excluding timestamp columns)
INSERT INTO users_new (id, email, password_hash, company_id, totp_secret)
SELECT id, email, password_hash, company_id, totp_secret FROM users;

-- Drop old table and rename new one
DROP TABLE users;
ALTER TABLE users_new RENAME TO users;

-- Re-enable foreign key constraints
PRAGMA foreign_keys = ON;

-- Recreate user triggers for the new table
CREATE TRIGGER users_insert_log 
AFTER INSERT ON users
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('users', NEW.id, 'create', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER users_update_log 
AFTER UPDATE ON users
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('users', NEW.id, 'update', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER users_delete_log 
AFTER DELETE ON users
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('users', OLD.id, 'delete', CURRENT_TIMESTAMP);
END;

-- Recreate session triggers
CREATE TRIGGER sessions_insert_log 
AFTER INSERT ON sessions
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('sessions', 0, 'create', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER sessions_update_log 
AFTER UPDATE ON sessions
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('sessions', 0, 'update', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER sessions_delete_log 
AFTER DELETE ON sessions
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('sessions', 0, 'delete', CURRENT_TIMESTAMP);
END;

-- Recreate other triggers that were dropped
CREATE TRIGGER IF NOT EXISTS enforce_newtown_roles_restriction
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
            JOIN companies c ON u.company_id = c.id 
            WHERE c.name = 'Newtown Energy'
        )
        THEN RAISE(ABORT, 'Newtown roles (newtown-admin, newtown-staff) can only be assigned to Newtown Energy company users')
    END;
END;

CREATE TRIGGER IF NOT EXISTS enforce_newtown_roles_restriction_update
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
            JOIN companies c ON u.company_id = c.id 
            WHERE c.name = 'Newtown Energy'
        )
        THEN RAISE(ABORT, 'Newtown roles (newtown-admin, newtown-staff) can only be assigned to Newtown Energy company users')
    END;
END;

CREATE TRIGGER IF NOT EXISTS prevent_user_without_roles
BEFORE DELETE ON user_roles
FOR EACH ROW
BEGIN
    SELECT CASE
        WHEN (SELECT COUNT(*) FROM user_roles WHERE user_id = OLD.user_id) = 1
        THEN RAISE(ABORT, 'Cannot remove the last role from a user. Users must have at least one role.')
    END;
END;

-- Recreate the users_without_roles view
CREATE VIEW users_without_roles AS
SELECT u.id, u.email, u.company_id
FROM users u
LEFT JOIN user_roles ur ON u.id = ur.user_id
WHERE ur.user_id IS NULL;