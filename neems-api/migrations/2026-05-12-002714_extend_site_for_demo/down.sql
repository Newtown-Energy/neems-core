-- SQLite cannot DROP COLUMN, so recreate sites without the demo fields and
-- restore the entity-activity triggers afterwards.

CREATE TABLE sites_backup (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    name TEXT NOT NULL,
    address TEXT NOT NULL,
    latitude REAL NOT NULL,
    longitude REAL NOT NULL,
    company_id INTEGER NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    ramp_duration_seconds INTEGER NOT NULL DEFAULT 120
);

INSERT INTO sites_backup (id, name, address, latitude, longitude, company_id, ramp_duration_seconds)
SELECT id, name, address, latitude, longitude, company_id, ramp_duration_seconds FROM sites;

DROP TABLE sites;

ALTER TABLE sites_backup RENAME TO sites;

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
