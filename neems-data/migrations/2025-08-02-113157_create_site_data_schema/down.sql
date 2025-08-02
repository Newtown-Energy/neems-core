-- Drop trigger first
DROP TRIGGER IF EXISTS update_sources_updated_at;

-- Drop indexes
DROP INDEX IF EXISTS idx_readings_source_recent;
DROP INDEX IF EXISTS idx_readings_timestamp;
DROP INDEX IF EXISTS idx_readings_source_time;

-- Drop tables (readings first due to foreign key)
DROP TABLE IF EXISTS readings;
DROP TABLE IF EXISTS sources;