-- Create schedule_templates table for reusable daily plans
CREATE TABLE schedule_templates (
    id INTEGER PRIMARY KEY NOT NULL,
    site_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    is_default BOOLEAN NOT NULL DEFAULT 0,
    is_active BOOLEAN NOT NULL DEFAULT 1,
    FOREIGN KEY(site_id) REFERENCES sites(id) ON DELETE CASCADE,
    UNIQUE(site_id, name)
);

-- Create indexes for performance
CREATE INDEX idx_schedule_templates_site ON schedule_templates(site_id);
CREATE INDEX idx_schedule_templates_site_default ON schedule_templates(site_id, is_default);

-- Add triggers for schedule_templates table to track entity activity
CREATE TRIGGER schedule_templates_insert_log
AFTER INSERT ON schedule_templates
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('schedule_templates', NEW.id, 'create', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER schedule_templates_update_log
AFTER UPDATE ON schedule_templates
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('schedule_templates', NEW.id, 'update', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER schedule_templates_delete_log
AFTER DELETE ON schedule_templates
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('schedule_templates', OLD.id, 'delete', CURRENT_TIMESTAMP);
END;
