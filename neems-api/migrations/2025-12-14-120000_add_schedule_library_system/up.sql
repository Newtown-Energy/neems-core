-- ============================================================================
-- Schedule Library System
-- ============================================================================

-- Battery command types (referenced by template entries)
CREATE TABLE schedule_commands (
    id INTEGER PRIMARY KEY NOT NULL,
    site_id INTEGER NOT NULL,
    type TEXT NOT NULL CHECK (type IN ('charge', 'discharge', 'trickle_charge')),
    parameters TEXT,  -- JSON-encoded parameters (optional, not used in UI yet)
    is_active BOOLEAN NOT NULL DEFAULT 1,
    FOREIGN KEY(site_id) REFERENCES sites(id) ON DELETE CASCADE
);

CREATE INDEX idx_schedule_commands_site ON schedule_commands(site_id);
CREATE INDEX idx_schedule_commands_site_active ON schedule_commands(site_id, is_active);

-- Reusable schedule templates (library items)
CREATE TABLE schedule_templates (
    id INTEGER PRIMARY KEY NOT NULL,
    site_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    is_active BOOLEAN NOT NULL DEFAULT 1,
    is_default BOOLEAN NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(site_id) REFERENCES sites(id) ON DELETE CASCADE,
    UNIQUE(site_id, name)
);

CREATE INDEX idx_schedule_templates_site ON schedule_templates(site_id);

-- Create unique index to ensure only one default per site
CREATE UNIQUE INDEX idx_schedule_templates_site_default
ON schedule_templates(site_id)
WHERE is_default = 1 AND is_active = 1;

-- Commands within a template (entries)
CREATE TABLE schedule_template_entries (
    id INTEGER PRIMARY KEY NOT NULL,
    template_id INTEGER NOT NULL,
    execution_offset_seconds INTEGER NOT NULL CHECK(execution_offset_seconds >= 0 AND execution_offset_seconds < 86400),
    schedule_command_id INTEGER NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT 1,
    FOREIGN KEY(template_id) REFERENCES schedule_templates(id) ON DELETE CASCADE,
    FOREIGN KEY(schedule_command_id) REFERENCES schedule_commands(id) ON DELETE RESTRICT
);

CREATE INDEX idx_schedule_template_entries_template ON schedule_template_entries(template_id);
CREATE INDEX idx_schedule_template_entries_offset ON schedule_template_entries(execution_offset_seconds);

-- Application rules for determining when schedules apply
CREATE TABLE application_rules (
    id INTEGER PRIMARY KEY NOT NULL,
    template_id INTEGER NOT NULL,
    rule_type TEXT NOT NULL CHECK (rule_type IN ('default', 'day_of_week', 'specific_date')),
    days_of_week TEXT,  -- JSON array: [0,1,2,3,4,5,6] where 0=Sunday
    specific_dates TEXT,  -- JSON array: ["2025-01-15", "2025-01-16"]
    override_reason TEXT,  -- Optional explanation for specific-date overrides
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(template_id) REFERENCES schedule_templates(id) ON DELETE CASCADE,
    CHECK (
        (rule_type = 'default' AND days_of_week IS NULL AND specific_dates IS NULL) OR
        (rule_type = 'day_of_week' AND days_of_week IS NOT NULL AND specific_dates IS NULL) OR
        (rule_type = 'specific_date' AND days_of_week IS NULL AND specific_dates IS NOT NULL)
    )
);

CREATE INDEX idx_application_rules_template ON application_rules(template_id);
CREATE INDEX idx_application_rules_type ON application_rules(rule_type);

-- ============================================================================
-- Entity Activity Triggers (for audit logging)
-- ============================================================================

-- schedule_commands triggers
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

-- schedule_templates triggers
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

-- schedule_template_entries triggers
CREATE TRIGGER schedule_template_entries_insert_log
AFTER INSERT ON schedule_template_entries
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('schedule_template_entries', NEW.id, 'create', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER schedule_template_entries_update_log
AFTER UPDATE ON schedule_template_entries
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('schedule_template_entries', NEW.id, 'update', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER schedule_template_entries_delete_log
AFTER DELETE ON schedule_template_entries
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('schedule_template_entries', OLD.id, 'delete', CURRENT_TIMESTAMP);
END;

-- application_rules triggers
CREATE TRIGGER application_rules_insert_log
AFTER INSERT ON application_rules
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('application_rules', NEW.id, 'create', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER application_rules_update_log
AFTER UPDATE ON application_rules
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('application_rules', NEW.id, 'update', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER application_rules_delete_log
AFTER DELETE ON application_rules
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('application_rules', OLD.id, 'delete', CURRENT_TIMESTAMP);
END;

-- ============================================================================
-- Default Schedule Constraints
-- ============================================================================

-- Trigger to prevent multiple defaults per site
CREATE TRIGGER prevent_multiple_defaults_insert
BEFORE INSERT ON schedule_templates
FOR EACH ROW
WHEN NEW.is_default = 1 AND NEW.is_active = 1
BEGIN
    SELECT CASE
        WHEN (SELECT COUNT(*) FROM schedule_templates
              WHERE site_id = NEW.site_id
              AND is_default = 1
              AND is_active = 1) > 0
        THEN RAISE(ABORT, 'A default schedule already exists for this site')
    END;
END;

CREATE TRIGGER prevent_multiple_defaults_update
BEFORE UPDATE ON schedule_templates
FOR EACH ROW
WHEN NEW.is_default = 1 AND NEW.is_active = 1
BEGIN
    SELECT CASE
        WHEN (SELECT COUNT(*) FROM schedule_templates
              WHERE site_id = NEW.site_id
              AND is_default = 1
              AND is_active = 1
              AND id != NEW.id) > 0
        THEN RAISE(ABORT, 'A default schedule already exists for this site')
    END;
END;
