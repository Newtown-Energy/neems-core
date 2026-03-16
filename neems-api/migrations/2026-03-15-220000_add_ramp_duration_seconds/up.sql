-- Add ramp_duration_seconds column to sites table
-- This configures the ConEd ramp rate constraint (time to ramp from 0 to full power)
-- Default is 120 seconds
ALTER TABLE sites ADD COLUMN ramp_duration_seconds INTEGER NOT NULL DEFAULT 120;
