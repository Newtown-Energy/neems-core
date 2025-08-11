-- Restore the original entity_activity table with the unique constraint
-- (This is mainly for reference - in practice we wouldn't want to restore the restrictive constraint)

-- Drop indexes
DROP INDEX IF EXISTS idx_entity_activity_operation;
DROP INDEX IF EXISTS idx_entity_activity_timestamp;
DROP INDEX IF EXISTS idx_entity_activity_table_entity;

-- Create temporary table with original structure (including the problematic unique constraint)
CREATE TABLE entity_activity_old (
    id INTEGER PRIMARY KEY NOT NULL,
    table_name TEXT NOT NULL,
    entity_id INTEGER NOT NULL,
    operation_type TEXT NOT NULL CHECK (operation_type IN ('create', 'update', 'delete')),
    timestamp TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    user_id INTEGER,
    UNIQUE (table_name, entity_id, operation_type, timestamp),
    FOREIGN KEY (user_id) REFERENCES users(id)
);

-- Copy existing data
INSERT INTO entity_activity_old 
SELECT id, table_name, entity_id, operation_type, timestamp, user_id 
FROM entity_activity;

-- Drop new table and rename old one
DROP TABLE entity_activity;
ALTER TABLE entity_activity_old RENAME TO entity_activity;

-- Recreate original indexes
CREATE INDEX idx_entity_activity_table_entity ON entity_activity(table_name, entity_id);
CREATE INDEX idx_entity_activity_timestamp ON entity_activity(timestamp);
CREATE INDEX idx_entity_activity_operation ON entity_activity(operation_type);