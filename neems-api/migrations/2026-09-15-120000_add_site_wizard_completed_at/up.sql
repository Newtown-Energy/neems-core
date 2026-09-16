-- When the site configuration wizard was first applied to this site.
--
-- The wizard is onboarding: it walks an operator through a site's power,
-- tariff windows and rebound floor, writes them to the row, and builds the
-- first peak-season schedule from them. Running it a second time is not a
-- re-run so much as a partial overwrite followed by a failure -- the site
-- settings are saved, then the schedule create is rejected for reusing a
-- name. The client needs to know onboarding is done so it can stop offering
-- it; nothing on `sites` said so.
--
-- Nullable because every existing row predates the column, and "never run"
-- is exactly what null should mean for them. Set once and never cleared:
-- what is worth keeping is when this site was first configured, not when
-- someone last pressed the button.

ALTER TABLE sites ADD COLUMN site_configuration_wizard_completed_at TIMESTAMP;
