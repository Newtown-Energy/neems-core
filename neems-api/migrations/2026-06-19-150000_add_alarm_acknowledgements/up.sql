-- Append-only log of alarm acknowledgements, tied to the acting user.
-- One row per acknowledgement (never updated/deleted in normal flow), so we
-- keep a full audit of who acknowledged which alarm and when. The API reads
-- the most recent row per alarm_num and combines it with the alarm data-state
-- (in the site DB) to compute latched visibility.
CREATE TABLE alarm_acknowledgements (
    id INTEGER PRIMARY KEY NOT NULL,
    alarm_num INTEGER NOT NULL,
    user_id INTEGER NOT NULL,
    acknowledged_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    note TEXT,
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- "Most recent ack per alarm" is the hot query.
CREATE INDEX idx_alarm_ack_alarm_num ON alarm_acknowledgements(alarm_num);
