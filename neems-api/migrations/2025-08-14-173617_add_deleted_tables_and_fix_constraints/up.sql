-- Create deleted_users table to preserve audit trail information
CREATE TABLE deleted_users (
    id INTEGER NOT NULL,  -- Original user ID
    email TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    company_id INTEGER NOT NULL,
    totp_secret TEXT,
    deleted_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_by INTEGER,  -- ID of user who performed the deletion (can be NULL for system deletions)
    PRIMARY KEY (id)
    -- No foreign key constraints - this is an archive table
);

-- Create deleted_companies table to preserve audit trail information
CREATE TABLE deleted_companies (
    id INTEGER NOT NULL,  -- Original company ID
    name TEXT NOT NULL,
    deleted_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    deleted_by INTEGER,  -- ID of user who performed the deletion (can be NULL for system deletions)
    PRIMARY KEY (id)
    -- No foreign key constraints - this is an archive table
);

-- Create indexes for performance on deleted tables
CREATE INDEX idx_deleted_users_deleted_at ON deleted_users(deleted_at);
CREATE INDEX idx_deleted_users_email ON deleted_users(email);
CREATE INDEX idx_deleted_companies_deleted_at ON deleted_companies(deleted_at);
CREATE INDEX idx_deleted_companies_name ON deleted_companies(name);

-- Recreate sessions table with CASCADE delete constraint
-- First drop the existing table and recreate with proper foreign key
DROP TABLE sessions;
CREATE TABLE sessions (
    id TEXT PRIMARY KEY NOT NULL,
    user_id INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL,
    expires_at TIMESTAMP,
    revoked BOOLEAN NOT NULL DEFAULT 0,
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Recreate sessions triggers that were lost when table was dropped
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
