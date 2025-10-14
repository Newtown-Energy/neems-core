-- Create commands table for atomic equipment actions
CREATE TABLE commands (
    id INTEGER PRIMARY KEY NOT NULL,
    site_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    equipment_type TEXT NOT NULL,
    equipment_id TEXT NOT NULL,
    action TEXT NOT NULL,
    parameters TEXT,
    is_active BOOLEAN NOT NULL DEFAULT 1,
    FOREIGN KEY(site_id) REFERENCES sites(id) ON DELETE CASCADE,
    UNIQUE(site_id, name)
);

-- Create indexes for performance
CREATE INDEX idx_commands_site ON commands(site_id);
CREATE INDEX idx_commands_site_active ON commands(site_id, is_active);

-- Add triggers for commands table to track entity activity
CREATE TRIGGER commands_insert_log
AFTER INSERT ON commands
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('commands', NEW.id, 'create', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER commands_update_log
AFTER UPDATE ON commands
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('commands', NEW.id, 'update', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER commands_delete_log
AFTER DELETE ON commands
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('commands', OLD.id, 'delete', CURRENT_TIMESTAMP);
END;
