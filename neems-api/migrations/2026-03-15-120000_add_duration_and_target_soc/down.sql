-- Revert: Remove duration_seconds and target_soc_percent from schedule_commands

-- Create old table structure
CREATE TABLE schedule_commands_old (
    id INTEGER PRIMARY KEY NOT NULL,
    site_id INTEGER NOT NULL,
    type TEXT NOT NULL CHECK (type IN ('charge', 'discharge', 'trickle_charge')),
    parameters TEXT,
    is_active BOOLEAN NOT NULL DEFAULT 1,
    FOREIGN KEY(site_id) REFERENCES sites(id) ON DELETE CASCADE
);

-- Copy data back (dropping new columns)
INSERT INTO schedule_commands_old (id, site_id, type, parameters, is_active)
SELECT id, site_id, type, parameters, is_active
FROM schedule_commands;

-- Drop existing triggers
DROP TRIGGER IF EXISTS schedule_commands_insert_log;
DROP TRIGGER IF EXISTS schedule_commands_update_log;
DROP TRIGGER IF EXISTS schedule_commands_delete_log;

-- Drop new table and rename old table
DROP TABLE schedule_commands;
ALTER TABLE schedule_commands_old RENAME TO schedule_commands;

-- Recreate indexes
CREATE INDEX idx_schedule_commands_site ON schedule_commands(site_id);
CREATE INDEX idx_schedule_commands_site_active ON schedule_commands(site_id, is_active);

-- Recreate triggers
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
