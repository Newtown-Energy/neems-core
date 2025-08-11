-- Create entity_activity table for centralized timestamp tracking
CREATE TABLE entity_activity (
    id INTEGER PRIMARY KEY NOT NULL,
    table_name TEXT NOT NULL,
    entity_id INTEGER NOT NULL,
    operation_type TEXT NOT NULL CHECK (operation_type IN ('create', 'update', 'delete')),
    timestamp TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    user_id INTEGER,
    UNIQUE (table_name, entity_id, operation_type, timestamp),
    FOREIGN KEY (user_id) REFERENCES users(id)
);

-- Create indexes for performance
CREATE INDEX idx_entity_activity_table_entity ON entity_activity(table_name, entity_id);
CREATE INDEX idx_entity_activity_timestamp ON entity_activity(timestamp);
CREATE INDEX idx_entity_activity_operation ON entity_activity(operation_type);