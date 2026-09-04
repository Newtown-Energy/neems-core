-- Operator requests to change a site input (a switch, a breaker, a relay).
--
-- A click on the single-line diagram is a *request to send a signal*, never a
-- state change. The resulting position is never stored here: that is read back
-- from the site through the digital points named in neems-data's
-- rtac::site_controls::SITE_CONTROLS, and remains the only authority on where a
-- breaker actually is. What this table records is the ask — who made it, for
-- which control, and whether the signal reached the RTAC.
--
-- Lifecycle: pending -> sent | failed
--   pending  an operator asked; not yet written to the RTAC
--   sent     the write to the RTAC succeeded. Terminal, and the success case:
--            getting the signal out is the whole of what this system owes.
--            Whether the breaker then moved is answered by its readback point,
--            separately and for as long as anyone cares to look.
--   failed   nothing wrote it to the RTAC — no write register is defined for
--            the control yet, the collector is not running, or the RTAC could
--            not be reached.
--
-- The E-stop keeps its own table (estop_requests): it is site-level rather
-- than per-element and engage-only, and sharing a vocabulary is not worth
-- rewriting the one control that matters most.

CREATE TABLE control_requests (
    id INTEGER PRIMARY KEY NOT NULL,
    site_id INTEGER NOT NULL REFERENCES sites(id),
    -- Matches SiteControl::id, which is also the React SLD component id. Text
    -- rather than a foreign key: the set of controls is a property of the
    -- site's equipment, versioned with the code that knows how to write them,
    -- not data an operator can edit.
    control_id TEXT NOT NULL,
    action TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    -- Nullable so the audit row survives the requesting user being deleted --
    -- ON DELETE SET NULL rather than the default NO ACTION, which would refuse
    -- the delete instead and make the row an obstacle rather than a record.
    requested_by INTEGER REFERENCES users(id) ON DELETE SET NULL,
    requested_at TIMESTAMP NOT NULL,
    sent_at TIMESTAMP,
    resolved_at TIMESTAMP,
    failure_reason TEXT
);

-- The collector polls for unresolved requests across a site on every tick.
CREATE INDEX idx_control_requests_site_status ON control_requests (site_id, status);

-- The diagram reads the most recent request per control to render its overlay.
CREATE INDEX idx_control_requests_site_control_requested_at
    ON control_requests (site_id, control_id, requested_at);

-- At most one pending request per control, enforced rather than merely
-- intended. `request_control` coalesces onto an existing pending request, but
-- that is a read followed by an insert; without this, two concurrent clicks
-- could both find nothing and both insert. The loser's row would then sit
-- pending past its timeout and finally be picked up as a second, unasked-for
-- signal — for a breaker, a second movement nobody requested.
CREATE UNIQUE INDEX idx_control_requests_one_pending_per_control
    ON control_requests (site_id, control_id)
    WHERE status = 'pending';
