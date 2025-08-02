-- Remove the four specific roles from the roles table

DELETE FROM roles
WHERE name IN ('newtown-admin', 'newtown-staff', 'admin', 'user');

DELETE FROM companies WHERE name = 'Newtown Energy';

