-- The E-stop is a physical button at the site and cannot be pressed
-- remotely. What an operator sends is a request for the site to shut down,
-- so the table is named for that. Rows and lifecycle are unchanged.
--
-- SQLite keeps an index's name across a table rename, so the indexes are
-- recreated under names that match.

ALTER TABLE estop_requests RENAME TO emergency_shutdown_requests;

DROP INDEX idx_estop_requests_site_status;
DROP INDEX idx_estop_requests_site_requested_at;
DROP INDEX idx_estop_requests_one_pending_per_site;

CREATE INDEX idx_emergency_shutdown_requests_site_status
    ON emergency_shutdown_requests (site_id, status);

CREATE INDEX idx_emergency_shutdown_requests_site_requested_at
    ON emergency_shutdown_requests (site_id, requested_at);

CREATE UNIQUE INDEX idx_emergency_shutdown_requests_one_pending_per_site
    ON emergency_shutdown_requests (site_id)
    WHERE status = 'pending';
