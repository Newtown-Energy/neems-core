-- Drop triggers for command_sets
DROP TRIGGER IF EXISTS command_sets_delete_log;
DROP TRIGGER IF EXISTS command_sets_update_log;
DROP TRIGGER IF EXISTS command_sets_insert_log;

-- Drop indexes
DROP INDEX IF EXISTS idx_command_set_commands_order;
DROP INDEX IF EXISTS idx_command_set_commands_set;
DROP INDEX IF EXISTS idx_command_sets_site_active;
DROP INDEX IF EXISTS idx_command_sets_site;

-- Drop tables
DROP TABLE IF EXISTS command_set_commands;
DROP TABLE IF EXISTS command_sets;
