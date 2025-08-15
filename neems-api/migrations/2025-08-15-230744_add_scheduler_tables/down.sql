-- Drop triggers
DROP TRIGGER IF EXISTS scheduler_overrides_delete_log;
DROP TRIGGER IF EXISTS scheduler_overrides_update_log;
DROP TRIGGER IF EXISTS scheduler_overrides_insert_log;
DROP TRIGGER IF EXISTS scheduler_scripts_delete_log;
DROP TRIGGER IF EXISTS scheduler_scripts_update_log;
DROP TRIGGER IF EXISTS scheduler_scripts_insert_log;

-- Drop indexes
DROP INDEX IF EXISTS idx_scheduler_executions_site_time;
DROP INDEX IF EXISTS idx_scheduler_overrides_active;
DROP INDEX IF EXISTS idx_scheduler_overrides_site_time;
DROP INDEX IF EXISTS idx_scheduler_scripts_site_active;

-- Drop tables in reverse order of dependency
DROP TABLE IF EXISTS scheduler_executions;
DROP TABLE IF EXISTS scheduler_overrides;
DROP TABLE IF EXISTS scheduler_scripts;
