-- Drop all schedule-related triggers
DROP TRIGGER IF EXISTS schedule_entries_delete_log;
DROP TRIGGER IF EXISTS schedule_entries_update_log;
DROP TRIGGER IF EXISTS schedule_entries_insert_log;

DROP TRIGGER IF EXISTS schedules_delete_log;
DROP TRIGGER IF EXISTS schedules_update_log;
DROP TRIGGER IF EXISTS schedules_insert_log;

DROP TRIGGER IF EXISTS schedule_template_entries_delete_log;
DROP TRIGGER IF EXISTS schedule_template_entries_update_log;
DROP TRIGGER IF EXISTS schedule_template_entries_insert_log;

DROP TRIGGER IF EXISTS schedule_templates_delete_log;
DROP TRIGGER IF EXISTS schedule_templates_update_log;
DROP TRIGGER IF EXISTS schedule_templates_insert_log;

DROP TRIGGER IF EXISTS schedule_commands_delete_log;
DROP TRIGGER IF EXISTS schedule_commands_update_log;
DROP TRIGGER IF EXISTS schedule_commands_insert_log;

DROP TRIGGER IF EXISTS scheduler_overrides_delete_log;
DROP TRIGGER IF EXISTS scheduler_overrides_update_log;
DROP TRIGGER IF EXISTS scheduler_overrides_insert_log;

DROP TRIGGER IF EXISTS scheduler_scripts_delete_log;
DROP TRIGGER IF EXISTS scheduler_scripts_update_log;
DROP TRIGGER IF EXISTS scheduler_scripts_insert_log;

-- Drop all schedule-related indexes
DROP INDEX IF EXISTS idx_schedule_entries_offset;
DROP INDEX IF EXISTS idx_schedule_entries_schedule;
DROP INDEX IF EXISTS idx_schedules_site_start;

DROP INDEX IF EXISTS idx_schedule_template_entries_offset;
DROP INDEX IF EXISTS idx_schedule_template_entries_template;
DROP INDEX IF EXISTS idx_schedule_templates_site_default;
DROP INDEX IF EXISTS idx_schedule_templates_site;

DROP INDEX IF EXISTS idx_schedule_commands_site_active;
DROP INDEX IF EXISTS idx_schedule_commands_site;

DROP INDEX IF EXISTS idx_scheduler_executions_site_time;
DROP INDEX IF EXISTS idx_scheduler_overrides_active;
DROP INDEX IF EXISTS idx_scheduler_overrides_site_time;
DROP INDEX IF EXISTS idx_scheduler_scripts_site_active;

-- Drop all schedule-related tables (in dependency order)
DROP TABLE IF EXISTS schedule_entries;
DROP TABLE IF EXISTS schedule_template_entries;
DROP TABLE IF EXISTS scheduler_executions;
DROP TABLE IF EXISTS schedules;
DROP TABLE IF EXISTS schedule_templates;
DROP TABLE IF EXISTS scheduler_overrides;
DROP TABLE IF EXISTS schedule_commands;
DROP TABLE IF EXISTS scheduler_scripts;
