-- Add CASCADE behavior to users.company_id foreign key constraint

-- Recreate users table with CASCADE delete for company_id
DROP TABLE users;
CREATE TABLE users (
    id INTEGER PRIMARY KEY NOT NULL,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    company_id INTEGER NOT NULL,
    totp_secret TEXT,
    FOREIGN KEY(company_id) REFERENCES companies(id) ON DELETE CASCADE
);

-- Recreate users triggers that were lost when table was dropped
CREATE TRIGGER users_insert_log 
AFTER INSERT ON users
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('users', NEW.id, 'create', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER users_update_log 
AFTER UPDATE ON users
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('users', NEW.id, 'update', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER users_delete_log 
AFTER DELETE ON users
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('users', OLD.id, 'delete', CURRENT_TIMESTAMP);
END;
