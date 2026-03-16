-- SQLite doesn't support DROP COLUMN directly, so we need to recreate the table
-- This migration removes the ramp_duration_seconds column from sites

-- Create a temporary table without the ramp_duration_seconds column
CREATE TABLE sites_backup (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    name TEXT NOT NULL,
    address TEXT NOT NULL,
    latitude REAL NOT NULL,
    longitude REAL NOT NULL,
    company_id INTEGER NOT NULL REFERENCES companies(id) ON DELETE CASCADE
);

-- Copy data to backup table
INSERT INTO sites_backup (id, name, address, latitude, longitude, company_id)
SELECT id, name, address, latitude, longitude, company_id FROM sites;

-- Drop the original table
DROP TABLE sites;

-- Rename backup to original
ALTER TABLE sites_backup RENAME TO sites;

-- Recreate triggers for entity_activity
CREATE TRIGGER IF NOT EXISTS track_sites_insert
AFTER INSERT ON sites
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('sites', NEW.id, 'create', datetime('now'));
END;

CREATE TRIGGER IF NOT EXISTS track_sites_update
AFTER UPDATE ON sites
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('sites', NEW.id, 'update', datetime('now'));
END;

CREATE TRIGGER IF NOT EXISTS track_sites_delete
AFTER DELETE ON sites
BEGIN
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp)
    VALUES ('sites', OLD.id, 'delete', datetime('now'));
END;
