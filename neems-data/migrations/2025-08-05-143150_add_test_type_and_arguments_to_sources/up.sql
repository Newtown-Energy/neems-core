-- Add test_type and arguments columns to support new collector system
ALTER TABLE sources ADD COLUMN test_type TEXT;
ALTER TABLE sources ADD COLUMN arguments TEXT; -- JSON string for key-value pairs

-- Migrate existing data from name field to new structure
-- ping_localhost -> test_type='ping', arguments='{"target":"127.0.0.1"}'
UPDATE sources 
SET test_type = 'ping', 
    arguments = '{"target":"127.0.0.1"}' 
WHERE name = 'ping_localhost';

-- charging_state -> test_type='charging_state', arguments='{}'
UPDATE sources 
SET test_type = 'charging_state', 
    arguments = '{}' 
WHERE name = 'charging_state';

-- disk_space -> test_type='disk_space', arguments='{}'
UPDATE sources 
SET test_type = 'disk_space', 
    arguments = '{}' 
WHERE name = 'disk_space';

-- charging_state_* -> test_type='charging_state', arguments='{"battery_id":"*"}'
UPDATE sources 
SET test_type = 'charging_state',
    arguments = '{"battery_id":"' || SUBSTR(name, 16) || '"}'
WHERE name LIKE 'charging_state_%';

-- ping_* -> test_type='ping', arguments='{"target":"*"}'  
UPDATE sources 
SET test_type = 'ping',
    arguments = '{"target":"' || SUBSTR(name, 6) || '"}'
WHERE name LIKE 'ping_%' AND name != 'ping_localhost';

-- Make new columns NOT NULL after data migration
-- First set any remaining NULL values to defaults (for unknown collector types)
UPDATE sources 
SET test_type = 'ping',
    arguments = '{"target":"' || name || '"}'
WHERE test_type IS NULL;

-- SQLite doesn't support ALTER COLUMN SET NOT NULL directly
-- We need to recreate the table with NOT NULL constraints
-- This will be handled by regenerating the schema with NOT NULL columns

-- Create index on test_type for performance
CREATE INDEX idx_sources_test_type ON sources (test_type);