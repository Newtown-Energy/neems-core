-- Site data sources table
CREATE TABLE sources (
    id INTEGER PRIMARY KEY,
    name TEXT UNIQUE NOT NULL,
    description TEXT,
    active BOOLEAN NOT NULL DEFAULT 1,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

-- Time-series readings table optimized for high-frequency writes
CREATE TABLE readings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id INTEGER NOT NULL REFERENCES sources(id),
    timestamp TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    data TEXT NOT NULL,              -- JSON data for flexible storage
    quality_flags INTEGER NOT NULL DEFAULT 0  -- Bitfield for quality indicators
);

-- Indexes optimized for time-series queries
CREATE INDEX idx_readings_source_time ON readings (source_id, timestamp);
CREATE INDEX idx_readings_timestamp ON readings (timestamp);
CREATE INDEX idx_readings_source_recent ON readings (source_id, timestamp DESC);

-- Trigger to update sources.updated_at
CREATE TRIGGER update_sources_updated_at 
    AFTER UPDATE ON sources
    FOR EACH ROW
BEGIN
    UPDATE sources SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
END;