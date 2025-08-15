-- Drop triggers first
DROP TRIGGER IF EXISTS devices_insert_log;
DROP TRIGGER IF EXISTS devices_update_log;
DROP TRIGGER IF EXISTS devices_delete_log;

-- Drop the devices table
DROP TABLE devices;