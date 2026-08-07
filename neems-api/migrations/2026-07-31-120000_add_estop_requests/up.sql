-- Operator-requested emergency stops.
--
-- E-stop *state* is never stored here. That is read from the RTAC (alarm 104,
-- see neems-data's rtac::alarm_definitions::ESTOP_ALARM_NUM) and remains the
-- only authority on whether a site is tripped. What this table records is the
-- *request*: who asked for a trip, when, whether the signal reached the RTAC,
-- and whether the RTAC ultimately confirmed it.
--
-- Lifecycle: pending -> dispatched | failed
--   pending     an operator asked for a trip; not yet written to the RTAC
--   dispatched  the collector wrote CommandType::EmergencyStop to the RTAC and
--               the write succeeded. Terminal, and the success case: getting
--               the signal out is the whole of what this system owes. Whether
--               the plant then tripped is answered by alarm 104, separately
--               and for as long as anyone cares to look.
--   failed      nothing wrote it to the RTAC within the timeout
--
-- Engage-only by design: there is no request to *clear* an E-stop. A latched
-- E-stop is cleared on site, after which alarm 104 drops and the observed
-- state follows on its own.

CREATE TABLE estop_requests (
    id INTEGER PRIMARY KEY NOT NULL,
    site_id INTEGER NOT NULL REFERENCES sites(id),
    status TEXT NOT NULL DEFAULT 'pending',
    -- Nullable so the audit row survives the requesting user being deleted.
    requested_by INTEGER REFERENCES users(id),
    requested_at TIMESTAMP NOT NULL,
    dispatched_at TIMESTAMP,
    resolved_at TIMESTAMP,
    failure_reason TEXT
);

-- The collector polls for the unresolved request per site on every tick.
CREATE INDEX idx_estop_requests_site_status ON estop_requests (site_id, status);

-- The status endpoint reads the most recent request for a site.
CREATE INDEX idx_estop_requests_site_requested_at ON estop_requests (site_id, requested_at);

-- At most one pending request per site, enforced rather than merely intended.
-- `request_estop` coalesces onto an existing pending request, but that is a
-- read followed by an insert; without this, two concurrent requests could both
-- find nothing and both insert. The loser's row would then never be read (the
-- collector only ever sees one), sit pending past its timeout, and finally be
-- picked up and signalled as a spurious trip once the winner resolved.
CREATE UNIQUE INDEX idx_estop_requests_one_pending_per_site
    ON estop_requests (site_id)
    WHERE status = 'pending';
