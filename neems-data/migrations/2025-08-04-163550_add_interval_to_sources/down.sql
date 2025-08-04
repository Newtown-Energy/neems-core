-- Remove the added columns
ALTER TABLE sources DROP COLUMN interval_seconds;
ALTER TABLE sources DROP COLUMN last_run;