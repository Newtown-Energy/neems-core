-- Restore timestamp columns to companies table
-- This recreates the original table structure

-- Create companies table with timestamp columns
CREATE TABLE companies_old (
    id INTEGER PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL
);

-- Copy existing data and add default timestamps
INSERT INTO companies_old (id, name, created_at, updated_at)
SELECT id, name, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP FROM companies;

-- Drop new table and rename old one
DROP TABLE companies;
ALTER TABLE companies_old RENAME TO companies;