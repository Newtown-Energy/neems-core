-- Add a free-form change reason captured at the API layer. NULL means
-- no reason supplied (the historic behavior and the trigger default).
-- We backfill this column from the orm layer, similar to how user_id
-- gets backfilled after the trigger-created row lands.
ALTER TABLE entity_activity ADD COLUMN change_reason TEXT NULL;
