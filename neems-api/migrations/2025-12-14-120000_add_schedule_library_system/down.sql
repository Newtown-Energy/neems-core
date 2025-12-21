-- Drop triggers
DROP TRIGGER IF EXISTS prevent_multiple_defaults_update;
DROP TRIGGER IF EXISTS prevent_multiple_defaults_insert;

DROP TRIGGER IF EXISTS application_rules_delete_log;
DROP TRIGGER IF EXISTS application_rules_update_log;
DROP TRIGGER IF EXISTS application_rules_insert_log;

DROP TRIGGER IF EXISTS schedule_template_entries_delete_log;
DROP TRIGGER IF EXISTS schedule_template_entries_update_log;
DROP TRIGGER IF EXISTS schedule_template_entries_insert_log;

DROP TRIGGER IF EXISTS schedule_templates_delete_log;
DROP TRIGGER IF EXISTS schedule_templates_update_log;
DROP TRIGGER IF EXISTS schedule_templates_insert_log;

DROP TRIGGER IF EXISTS schedule_commands_delete_log;
DROP TRIGGER IF EXISTS schedule_commands_update_log;
DROP TRIGGER IF EXISTS schedule_commands_insert_log;

-- Drop indexes
DROP INDEX IF EXISTS idx_application_rules_type;
DROP INDEX IF EXISTS idx_application_rules_template;

DROP INDEX IF EXISTS idx_schedule_template_entries_offset;
DROP INDEX IF EXISTS idx_schedule_template_entries_template;

DROP INDEX IF EXISTS idx_schedule_templates_site_default;
DROP INDEX IF EXISTS idx_schedule_templates_site;

DROP INDEX IF EXISTS idx_schedule_commands_site_active;
DROP INDEX IF EXISTS idx_schedule_commands_site;

-- Drop tables (in reverse dependency order)
DROP TABLE IF EXISTS application_rules;
DROP TABLE IF EXISTS schedule_template_entries;
DROP TABLE IF EXISTS schedule_templates;
DROP TABLE IF EXISTS schedule_commands;
