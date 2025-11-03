-- Drop triggers for schedule_templates
DROP TRIGGER IF EXISTS schedule_templates_delete_log;
DROP TRIGGER IF EXISTS schedule_templates_update_log;
DROP TRIGGER IF EXISTS schedule_templates_insert_log;

-- Drop indexes
DROP INDEX IF EXISTS idx_schedule_templates_site_default;
DROP INDEX IF EXISTS idx_schedule_templates_site;

-- Drop table
DROP TABLE IF EXISTS schedule_templates;
