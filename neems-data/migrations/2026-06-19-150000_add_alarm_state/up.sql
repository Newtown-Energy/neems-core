-- Persistent per-alarm data state, maintained by the RTAC collector
-- independently of acknowledgement. One row per alarm_num; the collector
-- upserts it on each rising/falling edge it observes. This lets the API
-- compute latched alarm visibility without rescanning all readings.
--
-- `data_active` is the current raw bit state. `last_rising_at` /
-- `last_falling_at` are the most recent false->true / true->false
-- transition timestamps (UTC). Both are NULL until the first edge of
-- that direction is seen.
CREATE TABLE alarm_state (
    alarm_num INTEGER PRIMARY KEY NOT NULL,
    data_active BOOLEAN NOT NULL DEFAULT 0,
    last_rising_at TIMESTAMP,
    last_falling_at TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
