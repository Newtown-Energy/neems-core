-- Rollback: Remove test_type and arguments columns
DROP INDEX IF EXISTS idx_sources_test_type;
ALTER TABLE sources DROP COLUMN test_type;
ALTER TABLE sources DROP COLUMN arguments;