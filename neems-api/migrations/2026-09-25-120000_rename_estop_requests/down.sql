ALTER TABLE emergency_shutdown_requests RENAME TO estop_requests;

DROP INDEX idx_emergency_shutdown_requests_site_status;
DROP INDEX idx_emergency_shutdown_requests_site_requested_at;
DROP INDEX idx_emergency_shutdown_requests_one_pending_per_site;

CREATE INDEX idx_estop_requests_site_status ON estop_requests (site_id, status);

CREATE INDEX idx_estop_requests_site_requested_at ON estop_requests (site_id, requested_at);

CREATE UNIQUE INDEX idx_estop_requests_one_pending_per_site
    ON estop_requests (site_id)
    WHERE status = 'pending';
