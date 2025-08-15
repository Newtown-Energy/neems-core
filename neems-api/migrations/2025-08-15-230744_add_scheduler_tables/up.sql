-- Create scheduler_scripts table for storing Lua scripts per site
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
