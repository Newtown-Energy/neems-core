-- Create schedules table for time-specific schedule instances
CREATE TABLE schedules (
    id INTEGER PRIMARY KEY NOT NULL,
    site_id INTEGER NOT NULL,
    schedule_start TIMESTAMP NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT 1,
    FOREIGN KEY(site_id) REFERENCES sites(id) ON DELETE CASCADE,
    UNIQUE(site_id, schedule_start)
);

-- Create schedule_entries table for specific scheduled events
CREATE TABLE schedule_entries (
    id INTEGER PRIMARY KEY NOT NULL,
    schedule_id INTEGER NOT NULL,
    execution_offset_seconds INTEGER NOT NULL CHECK(execution_offset_seconds >= 0),
    schedule_command_id INTEGER NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT 1,
    FOREIGN KEY(schedule_id) REFERENCES schedules(id) ON DELETE CASCADE,
    FOREIGN KEY(schedule_command_id) REFERENCES schedule_commands(id) ON DELETE RESTRICT
);

-- Create indexes for performance
CREATE INDEX idx_schedules_site_start ON schedules(site_id, schedule_start);
CREATE INDEX idx_schedule_entries_schedule ON schedule_entries(schedule_id);
CREATE INDEX idx_schedule_entries_offset ON schedule_entries(execution_offset_seconds);

-- Add triggers for schedules table to track entity activity
CREATE TRIGGER schedules_insert_log
AFTER INSERT ON schedules
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('schedules', NEW.id, 'create', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER schedules_update_log
AFTER UPDATE ON schedules
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('schedules', NEW.id, 'update', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER schedules_delete_log
AFTER DELETE ON schedules
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('schedules', OLD.id, 'delete', CURRENT_TIMESTAMP);
END;

-- Add triggers for schedule_entries table to track entity activity
CREATE TRIGGER schedule_entries_insert_log
AFTER INSERT ON schedule_entries
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('schedule_entries', NEW.id, 'create', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER schedule_entries_update_log
AFTER UPDATE ON schedule_entries
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('schedule_entries', NEW.id, 'update', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER schedule_entries_delete_log
AFTER DELETE ON schedule_entries
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('schedule_entries', OLD.id, 'delete', CURRENT_TIMESTAMP);
END;
