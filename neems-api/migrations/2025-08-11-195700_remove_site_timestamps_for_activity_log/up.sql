-- Remove timestamps from sites table since they're now tracked in entity_activity
-- We need to temporarily drop and recreate triggers to avoid conflicts during ALTER TABLE

-- Drop existing triggers temporarily
DROP TRIGGER IF EXISTS sites_insert_log;
DROP TRIGGER IF EXISTS sites_update_log;
DROP TRIGGER IF EXISTS sites_delete_log;

-- Disable foreign key constraints temporarily for safe schema changes
PRAGMA foreign_keys = OFF;

-- Create new sites table without timestamps
CREATE TABLE sites_new (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    name VARCHAR NOT NULL,
    address VARCHAR NOT NULL,
    latitude REAL NOT NULL,
    longitude REAL NOT NULL,
    company_id INTEGER NOT NULL,
    FOREIGN KEY(company_id) REFERENCES companies(id)
);

-- Copy data from old table to new table (excluding timestamps)
INSERT INTO sites_new (id, name, address, latitude, longitude, company_id)
SELECT id, name, address, latitude, longitude, company_id FROM sites;

-- Drop the old table
DROP TABLE sites;

-- Rename new table to original name
ALTER TABLE sites_new RENAME TO sites;

-- Re-enable foreign key constraints
PRAGMA foreign_keys = ON;

-- Recreate the triggers for sites table
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
