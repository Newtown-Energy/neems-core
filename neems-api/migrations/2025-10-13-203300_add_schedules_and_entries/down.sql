-- Drop triggers for schedule_entries
DROP TRIGGER IF EXISTS schedule_entries_delete_log;
DROP TRIGGER IF EXISTS schedule_entries_update_log;
DROP TRIGGER IF EXISTS schedule_entries_insert_log;

-- Drop triggers for schedules
DROP TRIGGER IF EXISTS schedules_delete_log;
DROP TRIGGER IF EXISTS schedules_update_log;
DROP TRIGGER IF EXISTS schedules_insert_log;

-- Drop indexes
DROP INDEX IF EXISTS idx_schedule_entries_execution_time;
DROP INDEX IF EXISTS idx_schedule_entries_template;
DROP INDEX IF EXISTS idx_schedule_entries_schedule;
DROP INDEX IF EXISTS idx_schedules_site_date;

-- Drop tables in reverse order of dependencies
DROP TABLE IF EXISTS schedule_entries;
DROP TABLE IF EXISTS schedules;
