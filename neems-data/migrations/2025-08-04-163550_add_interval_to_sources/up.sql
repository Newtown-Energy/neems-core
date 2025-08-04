-- Add interval_seconds column to sources table
ALTER TABLE sources ADD COLUMN interval_seconds INTEGER NOT NULL DEFAULT 1;

-- Add last_run column to track when each source was last started
ALTER TABLE sources ADD COLUMN last_run TIMESTAMP;