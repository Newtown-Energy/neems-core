-- Rename the 'user' role to 'staff'
-- This is a simple name change that preserves all existing relationships

UPDATE roles 
SET name = 'staff'
WHERE name = 'user';