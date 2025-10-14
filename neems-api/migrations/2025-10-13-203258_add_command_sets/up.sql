-- Create command_sets table for reusable command workflows
CREATE TABLE command_sets (
    id INTEGER PRIMARY KEY NOT NULL,
    site_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    is_active BOOLEAN NOT NULL DEFAULT 1,
    FOREIGN KEY(site_id) REFERENCES sites(id) ON DELETE CASCADE,
    UNIQUE(site_id, name)
);

-- Create junction table for commands within command sets
CREATE TABLE command_set_commands (
    command_set_id INTEGER NOT NULL,
    command_id INTEGER NOT NULL,
    execution_order INTEGER NOT NULL,
    delay_ms INTEGER,
    condition TEXT,
    PRIMARY KEY(command_set_id, command_id),
    FOREIGN KEY(command_set_id) REFERENCES command_sets(id) ON DELETE CASCADE,
    FOREIGN KEY(command_id) REFERENCES commands(id) ON DELETE CASCADE
);

-- Create indexes for performance
CREATE INDEX idx_command_sets_site ON command_sets(site_id);
CREATE INDEX idx_command_sets_site_active ON command_sets(site_id, is_active);
CREATE INDEX idx_command_set_commands_set ON command_set_commands(command_set_id);
CREATE INDEX idx_command_set_commands_order ON command_set_commands(command_set_id, execution_order);

-- Add triggers for command_sets table to track entity activity
CREATE TRIGGER command_sets_insert_log
AFTER INSERT ON command_sets
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('command_sets', NEW.id, 'create', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER command_sets_update_log
AFTER UPDATE ON command_sets
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('command_sets', NEW.id, 'update', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER command_sets_delete_log
AFTER DELETE ON command_sets
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('command_sets', OLD.id, 'delete', CURRENT_TIMESTAMP);
END;
