-- Add triggers for companies table
CREATE TRIGGER companies_insert_log 
AFTER INSERT ON companies
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('companies', NEW.id, 'create', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER companies_update_log 
AFTER UPDATE ON companies
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('companies', NEW.id, 'update', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER companies_delete_log 
AFTER DELETE ON companies
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('companies', OLD.id, 'delete', CURRENT_TIMESTAMP);
END;

-- Add triggers for sites table
CREATE TRIGGER sites_insert_log 
AFTER INSERT ON sites
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('sites', NEW.id, 'create', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER sites_update_log 
AFTER UPDATE ON sites
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('sites', NEW.id, 'update', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER sites_delete_log 
AFTER DELETE ON sites
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('sites', OLD.id, 'delete', CURRENT_TIMESTAMP);
END;

-- Add triggers for sessions table
-- Sessions use string IDs, so we'll use 0 as placeholder entity_id
-- We track session activity but can't lookup by specific session ID
CREATE TRIGGER sessions_insert_log 
AFTER INSERT ON sessions
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('sessions', 0, 'create', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER sessions_update_log 
AFTER UPDATE ON sessions
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('sessions', 0, 'update', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER sessions_delete_log 
AFTER DELETE ON sessions
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('sessions', 0, 'delete', CURRENT_TIMESTAMP);
END;