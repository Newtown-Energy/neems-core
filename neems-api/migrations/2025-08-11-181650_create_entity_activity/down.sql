-- Drop indexes
DROP INDEX IF EXISTS idx_entity_activity_operation;
DROP INDEX IF EXISTS idx_entity_activity_timestamp;
DROP INDEX IF EXISTS idx_entity_activity_table_entity;

-- Drop entity_activity table
DROP TABLE entity_activity;