-- Rename the 'staff' role back to 'user'
-- This reverses the migration by changing the name back

UPDATE roles 
SET name = 'user'
WHERE name = 'staff';