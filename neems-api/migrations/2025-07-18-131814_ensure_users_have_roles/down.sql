-- Remove the trigger that prevents users from having no roles
DROP TRIGGER IF EXISTS prevent_user_without_roles;

-- Remove the monitoring view
DROP VIEW IF EXISTS users_without_roles;

-- Note: We don't remove the user_roles assignments created in the up migration
-- as that would potentially break existing functionality and relationships