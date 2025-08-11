-- Temporarily drop triggers to avoid issues during table recreation
DROP TRIGGER IF EXISTS users_delete_log;
DROP TRIGGER IF EXISTS users_update_log;  
DROP TRIGGER IF EXISTS users_insert_log;

-- Recreate entity_activity table without the overly restrictive unique constraint
-- Create temporary table with correct structure
CREATE TABLE entity_activity_new (
    id INTEGER PRIMARY KEY NOT NULL,
    table_name TEXT NOT NULL,
    entity_id INTEGER NOT NULL,
    operation_type TEXT NOT NULL CHECK (operation_type IN ('create', 'update', 'delete')),
    timestamp TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    user_id INTEGER,
    FOREIGN KEY (user_id) REFERENCES users(id)
);

-- Copy existing data
INSERT INTO entity_activity_new 
SELECT id, table_name, entity_id, operation_type, timestamp, user_id 
FROM entity_activity;

-- Drop old table and rename new one
DROP TABLE entity_activity;
ALTER TABLE entity_activity_new RENAME TO entity_activity;

-- Recreate indexes for performance (but without unique constraint)
CREATE INDEX idx_entity_activity_table_entity ON entity_activity(table_name, entity_id);
CREATE INDEX idx_entity_activity_timestamp ON entity_activity(timestamp);
CREATE INDEX idx_entity_activity_operation ON entity_activity(operation_type);

-- Recreate the triggers
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