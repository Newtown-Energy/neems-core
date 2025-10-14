-- Create schedules table for date-specific schedule instances
CREATE TABLE schedules (
    id INTEGER PRIMARY KEY NOT NULL,
    site_id INTEGER NOT NULL,
    template_id INTEGER,
    schedule_date DATE NOT NULL,
    is_custom BOOLEAN NOT NULL DEFAULT 0,
    is_active BOOLEAN NOT NULL DEFAULT 1,
    FOREIGN KEY(site_id) REFERENCES sites(id) ON DELETE CASCADE,
    FOREIGN KEY(template_id) REFERENCES schedule_templates(id) ON DELETE SET NULL,
    UNIQUE(site_id, schedule_date)
);

-- Create schedule_entries table for specific scheduled events
CREATE TABLE schedule_entries (
    id INTEGER PRIMARY KEY NOT NULL,
    schedule_id INTEGER,
    template_id INTEGER,
    execution_time TIME NOT NULL,
    end_time TIME,
    command_id INTEGER,
    command_set_id INTEGER,
    condition TEXT,
    is_active BOOLEAN NOT NULL DEFAULT 1,
    FOREIGN KEY(schedule_id) REFERENCES schedules(id) ON DELETE CASCADE,
    FOREIGN KEY(template_id) REFERENCES schedule_templates(id) ON DELETE CASCADE,
    FOREIGN KEY(command_id) REFERENCES commands(id) ON DELETE RESTRICT,
    FOREIGN KEY(command_set_id) REFERENCES command_sets(id) ON DELETE RESTRICT,
    CHECK((schedule_id IS NULL) != (template_id IS NULL)),
    CHECK((command_id IS NULL) != (command_set_id IS NULL)),
    CHECK((end_time IS NULL) OR (end_time > execution_time))
);

-- Create indexes for performance
CREATE INDEX idx_schedules_site_date ON schedules(site_id, schedule_date);
CREATE INDEX idx_schedule_entries_schedule ON schedule_entries(schedule_id);
CREATE INDEX idx_schedule_entries_template ON schedule_entries(template_id);
CREATE INDEX idx_schedule_entries_execution_time ON schedule_entries(execution_time);

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
