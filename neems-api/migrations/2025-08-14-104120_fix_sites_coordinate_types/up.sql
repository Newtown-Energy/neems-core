-- Fix coordinate column types to use DOUBLE PRECISION instead of REAL
-- This ensures Diesel generates the correct Double type instead of Float

PRAGMA foreign_keys = OFF;

-- Create new sites table with correct types
CREATE TABLE sites_new (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    name VARCHAR NOT NULL,
    address VARCHAR NOT NULL,
    latitude DOUBLE PRECISION NOT NULL,
    longitude DOUBLE PRECISION NOT NULL,
    company_id INTEGER NOT NULL,
    FOREIGN KEY(company_id) REFERENCES companies(id)
);

-- Copy all data from existing sites table
INSERT INTO sites_new SELECT * FROM sites;

-- Drop the old table
DROP TABLE sites;

-- Rename new table to sites
ALTER TABLE sites_new RENAME TO sites;

-- Recreate all triggers for sites table
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

PRAGMA foreign_keys = ON;