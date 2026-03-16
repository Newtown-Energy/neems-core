-- Add duration_seconds and target_soc_percent to schedule_commands
-- SQLite requires table recreation to add columns with CHECK constraints

-- Create new table with additional columns
CREATE TABLE schedule_commands_new (
    id INTEGER PRIMARY KEY NOT NULL,
    site_id INTEGER NOT NULL,
    type TEXT NOT NULL CHECK (type IN ('charge', 'discharge', 'trickle_charge')),
    parameters TEXT,
    duration_seconds INTEGER CHECK (duration_seconds IS NULL OR duration_seconds > 0),
    target_soc_percent INTEGER CHECK (target_soc_percent IS NULL OR (target_soc_percent >= 0 AND target_soc_percent <= 100)),
    is_active BOOLEAN NOT NULL DEFAULT 1,
    FOREIGN KEY(site_id) REFERENCES sites(id) ON DELETE CASCADE
);

-- Copy existing data (new columns get NULL by default)
INSERT INTO schedule_commands_new (id, site_id, type, parameters, duration_seconds, target_soc_percent, is_active)
SELECT id, site_id, type, parameters, NULL, NULL, is_active
FROM schedule_commands;

-- Drop existing triggers before dropping table
DROP TRIGGER IF EXISTS schedule_commands_insert_log;
DROP TRIGGER IF EXISTS schedule_commands_update_log;
DROP TRIGGER IF EXISTS schedule_commands_delete_log;

-- Drop old table and rename new table
DROP TABLE schedule_commands;
ALTER TABLE schedule_commands_new RENAME TO schedule_commands;

-- Recreate indexes
CREATE INDEX idx_schedule_commands_site ON schedule_commands(site_id);
CREATE INDEX idx_schedule_commands_site_active ON schedule_commands(site_id, is_active);

-- Recreate triggers for entity activity tracking
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
