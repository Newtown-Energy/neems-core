-- Trigger to log user creation
CREATE TRIGGER users_insert_log 
AFTER INSERT ON users
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('users', NEW.id, 'create', CURRENT_TIMESTAMP);
END;

-- Trigger to log user updates  
CREATE TRIGGER users_update_log 
AFTER UPDATE ON users
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('users', NEW.id, 'update', CURRENT_TIMESTAMP);
END;

-- Trigger to log user deletion
CREATE TRIGGER users_delete_log 
AFTER DELETE ON users
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('users', OLD.id, 'delete', CURRENT_TIMESTAMP);
END;