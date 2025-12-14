-- Recreate scheduler_scripts table for storing Lua scripts per site
CREATE TABLE scheduler_scripts (
    id INTEGER PRIMARY KEY NOT NULL,
    site_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    script_content TEXT NOT NULL,
    language TEXT NOT NULL DEFAULT 'lua' CHECK (language IN ('lua')),
    is_active BOOLEAN NOT NULL DEFAULT 1,
    version INTEGER NOT NULL DEFAULT 1,
    FOREIGN KEY(site_id) REFERENCES sites(id) ON DELETE CASCADE,
    UNIQUE(site_id, name, version)
);

-- Create scheduler_overrides table for temporary state overrides
CREATE TABLE scheduler_overrides (
    id INTEGER PRIMARY KEY NOT NULL,
    site_id INTEGER NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('charge', 'discharge', 'idle')),
    start_time TIMESTAMP NOT NULL,
    end_time TIMESTAMP NOT NULL,
    created_by INTEGER NOT NULL,
    reason TEXT,
    is_active BOOLEAN NOT NULL DEFAULT 1,
    FOREIGN KEY(site_id) REFERENCES sites(id) ON DELETE CASCADE,
    FOREIGN KEY(created_by) REFERENCES users(id) ON DELETE RESTRICT,
    CHECK(end_time > start_time)
);

-- Create scheduler_executions table for execution history (optional)
CREATE TABLE scheduler_executions (
    id INTEGER PRIMARY KEY NOT NULL,
    site_id INTEGER NOT NULL,
    script_id INTEGER,
    override_id INTEGER,
    execution_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    state_result TEXT NOT NULL CHECK (state_result IN ('charge', 'discharge', 'idle')),
    execution_duration_ms INTEGER,
    error_message TEXT,
    FOREIGN KEY(site_id) REFERENCES sites(id) ON DELETE CASCADE,
    FOREIGN KEY(script_id) REFERENCES scheduler_scripts(id) ON DELETE SET NULL,
    FOREIGN KEY(override_id) REFERENCES scheduler_overrides(id) ON DELETE SET NULL
);

-- Create indexes for performance
CREATE INDEX idx_scheduler_scripts_site_active ON scheduler_scripts(site_id, is_active);
CREATE INDEX idx_scheduler_overrides_site_time ON scheduler_overrides(site_id, start_time, end_time);
CREATE INDEX idx_scheduler_overrides_active ON scheduler_overrides(is_active);
CREATE INDEX idx_scheduler_executions_site_time ON scheduler_executions(site_id, execution_time);

-- Add triggers for scheduler_scripts table to track entity activity
CREATE TRIGGER scheduler_scripts_insert_log
AFTER INSERT ON scheduler_scripts
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('scheduler_scripts', NEW.id, 'create', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER scheduler_scripts_update_log
AFTER UPDATE ON scheduler_scripts
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('scheduler_scripts', NEW.id, 'update', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER scheduler_scripts_delete_log
AFTER DELETE ON scheduler_scripts
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('scheduler_scripts', OLD.id, 'delete', CURRENT_TIMESTAMP);
END;

-- Add triggers for scheduler_overrides table to track entity activity
CREATE TRIGGER scheduler_overrides_insert_log
AFTER INSERT ON scheduler_overrides
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('scheduler_overrides', NEW.id, 'create', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER scheduler_overrides_update_log
AFTER UPDATE ON scheduler_overrides
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('scheduler_overrides', NEW.id, 'update', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER scheduler_overrides_delete_log
AFTER DELETE ON scheduler_overrides
FOR EACH ROW
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('scheduler_overrides', OLD.id, 'delete', CURRENT_TIMESTAMP);
END;

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

-- Create schedule_template_entries table for template entry definitions
CREATE TABLE schedule_template_entries (
    id INTEGER PRIMARY KEY NOT NULL,
    template_id INTEGER NOT NULL,
    execution_offset_seconds INTEGER NOT NULL CHECK(execution_offset_seconds >= 0),
    schedule_command_id INTEGER NOT NULL,
    is_active BOOLEAN NOT NULL DEFAULT 1,
    FOREIGN KEY(template_id) REFERENCES schedule_templates(id) ON DELETE CASCADE,
    FOREIGN KEY(schedule_command_id) REFERENCES schedule_commands(id) ON DELETE RESTRICT
);

-- Create indexes for performance
CREATE INDEX idx_schedule_template_entries_template ON schedule_template_entries(template_id);
CREATE INDEX idx_schedule_template_entries_offset ON schedule_template_entries(execution_offset_seconds);

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

-- Add triggers for schedule_template_entries table to track entity activity
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
