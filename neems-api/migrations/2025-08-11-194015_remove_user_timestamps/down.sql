-- Restore timestamp columns to users table
-- This recreates the original table structure

-- Drop all triggers first
DROP TRIGGER IF EXISTS users_delete_log;
DROP TRIGGER IF EXISTS users_update_log; 
DROP TRIGGER IF EXISTS users_insert_log;
DROP TRIGGER IF EXISTS enforce_newtown_roles_restriction;
DROP TRIGGER IF EXISTS enforce_newtown_roles_restriction_update;
DROP TRIGGER IF EXISTS prevent_user_without_roles;
DROP TRIGGER IF EXISTS sessions_insert_log;
DROP TRIGGER IF EXISTS sessions_update_log;
DROP TRIGGER IF EXISTS sessions_delete_log;

PRAGMA foreign_keys = OFF;

-- Create users table with timestamp columns
CREATE TABLE users_old (
    id INTEGER PRIMARY KEY NOT NULL,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    company_id INTEGER NOT NULL REFERENCES companies(id),
    totp_secret TEXT
);

-- Copy existing data and add default timestamps
INSERT INTO users_old (id, email, password_hash, created_at, updated_at, company_id, totp_secret)
SELECT id, email, password_hash, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, company_id, totp_secret FROM users;

-- Drop new table and rename old one
DROP TABLE users;
ALTER TABLE users_old RENAME TO users;

PRAGMA foreign_keys = ON;

-- Recreate original triggers
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