-- Per-site trickle-charge power limit (kW). Used as the commanded
-- power level whenever a schedule emits a `trickle_charge` command.
-- Nullable so existing rows are left as "no limit configured"; the
-- consuming code falls back to a sensible default when null.

ALTER TABLE sites ADD COLUMN trickle_charge_power_kw DOUBLE PRECISION;
