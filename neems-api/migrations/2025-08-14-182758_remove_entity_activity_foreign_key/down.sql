-- Restore foreign key constraint to entity_activity table
-- (This reverses the change but is not recommended for production)

DROP TABLE entity_activity;
CREATE TABLE entity_activity (
    id INTEGER PRIMARY KEY NOT NULL,
    table_name TEXT NOT NULL,
    entity_id INTEGER NOT NULL,
    operation_type TEXT NOT NULL CHECK (operation_type IN ('create', 'update', 'delete')),
    timestamp TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    user_id INTEGER,
    FOREIGN KEY (user_id) REFERENCES users(id)
);

-- Recreate indexes
CREATE INDEX idx_entity_activity_table_entity ON entity_activity(table_name, entity_id);
CREATE INDEX idx_entity_activity_timestamp ON entity_activity(timestamp);
CREATE INDEX idx_entity_activity_operation ON entity_activity(operation_type);
