-- Extend sites table with demo-driven configuration fields.
-- These are nullable where the demo wizard would supply a real value at
-- configuration time, and have safe NOT NULL defaults where the field
-- governs system behavior (closed_loop_enabled, rebound floor, variant).

-- Use DOUBLE PRECISION (not REAL) so that the Diesel CLI infers `Double`
-- rather than `Float` when regenerating schema.rs. This matches the
-- existing latitude/longitude convention on this table.
ALTER TABLE sites ADD COLUMN power_kw DOUBLE PRECISION;
ALTER TABLE sites ADD COLUMN capacity_kwh DOUBLE PRECISION;
ALTER TABLE sites ADD COLUMN closed_loop_enabled BOOLEAN NOT NULL DEFAULT 1;
ALTER TABLE sites ADD COLUMN off_peak_start_minutes INTEGER;
ALTER TABLE sites ADD COLUMN off_peak_end_minutes INTEGER;
ALTER TABLE sites ADD COLUMN peak_revenue_start_minutes INTEGER;
ALTER TABLE sites ADD COLUMN peak_revenue_end_minutes INTEGER;
ALTER TABLE sites ADD COLUMN interconnection_max_output_kw DOUBLE PRECISION;
ALTER TABLE sites ADD COLUMN rebound_protection_soc_floor_percent DOUBLE PRECISION NOT NULL DEFAULT 2.0;
ALTER TABLE sites ADD COLUMN site_variant TEXT NOT NULL DEFAULT 'standard';
