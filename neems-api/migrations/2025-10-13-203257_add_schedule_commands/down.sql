-- Drop triggers for commands
DROP TRIGGER IF EXISTS commands_delete_log;
DROP TRIGGER IF EXISTS commands_update_log;
DROP TRIGGER IF EXISTS commands_insert_log;

-- Drop indexes
DROP INDEX IF EXISTS idx_commands_site_active;
DROP INDEX IF EXISTS idx_commands_site;

-- Drop table
DROP TABLE IF EXISTS commands;
