-- Create devices table
CREATE TABLE devices (
    id INTEGER PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    type TEXT NOT NULL,
    model TEXT NOT NULL,
    serial TEXT,
    ip_address TEXT,
    install_date TIMESTAMP,
    company_id INTEGER NOT NULL,
    site_id INTEGER NOT NULL,
    FOREIGN KEY(company_id) REFERENCES companies(id) ON DELETE CASCADE,
    FOREIGN KEY(site_id) REFERENCES sites(id) ON DELETE CASCADE,
    UNIQUE(company_id, site_id, name)
);

-- Add triggers for devices table to track entity activity
CREATE TRIGGER devices_insert_log 
AFTER INSERT ON devices
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('devices', NEW.id, 'create', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER devices_update_log 
AFTER UPDATE ON devices
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('devices', NEW.id, 'update', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER devices_delete_log 
AFTER DELETE ON devices
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('devices', OLD.id, 'delete', CURRENT_TIMESTAMP);
END;