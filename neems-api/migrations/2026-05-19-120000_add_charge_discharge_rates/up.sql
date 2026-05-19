-- Add per-site charge and discharge rate caps, expressed as a
-- percentage of the site's `power_kw`. Demo Script v2 wants the
-- calendar's day-cell bars to scale visually with these (charge
-- typically ~50%, discharge ~100% for the showcase site).
--
-- DOUBLE PRECISION matches the latitude/longitude/power_kw convention
-- on this table so Diesel CLI infers `Double`. Default 100 means
-- "no extra cap beyond power_kw" — existing rows are unchanged in
-- behavior.

ALTER TABLE sites ADD COLUMN charge_rate_percent DOUBLE PRECISION NOT NULL DEFAULT 100;
ALTER TABLE sites ADD COLUMN discharge_rate_percent DOUBLE PRECISION NOT NULL DEFAULT 100;
