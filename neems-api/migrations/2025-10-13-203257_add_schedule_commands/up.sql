-- Create schedule_commands table for battery charging commands
CREATE TABLE schedule_commands (
    id INTEGER PRIMARY KEY NOT NULL,
    site_id INTEGER NOT NULL,
    type TEXT NOT NULL CHECK (type IN ('charge', 'discharge', 'trickle_charge')),
    parameters TEXT,
    is_active BOOLEAN NOT NULL DEFAULT 1,
    FOREIGN KEY(site_id) REFERENCES sites(id) ON DELETE CASCADE
);

-- Create indexes for performance
CREATE INDEX idx_schedule_commands_site ON schedule_commands(site_id);
CREATE INDEX idx_schedule_commands_site_active ON schedule_commands(site_id, is_active);

-- Add triggers for schedule_commands table to track entity activity
CREATE TRIGGER schedule_commands_insert_log
AFTER INSERT ON schedule_commands
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('schedule_commands', NEW.id, 'create', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER schedule_commands_update_log
AFTER UPDATE ON schedule_commands
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('schedule_commands', NEW.id, 'update', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER schedule_commands_delete_log
AFTER DELETE ON schedule_commands
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('schedule_commands', OLD.id, 'delete', CURRENT_TIMESTAMP);
END;
