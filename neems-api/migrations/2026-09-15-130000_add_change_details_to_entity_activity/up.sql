-- Machine-readable description of what an operator changed, stored as
-- a JSON `ChangeDetails` blob. NULL means nothing was recorded — every
-- row written before this migration, plus trigger-only writes that go
-- through no API handler.
--
-- Backfilled from the orm layer after the trigger row lands, exactly
-- like `user_id` and `change_reason`.
ALTER TABLE entity_activity ADD COLUMN change_details TEXT NULL;
