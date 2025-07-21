-- First, assign a default role to any existing users without roles
-- This ensures we don't violate the constraint we're about to create
INSERT INTO user_roles (user_id, role_id)
SELECT u.id, r.id
FROM users u
JOIN roles r ON r.name = 'user'
WHERE u.id NOT IN (SELECT DISTINCT user_id FROM user_roles);

-- Create a trigger to ensure users always have at least one role
-- This trigger will prevent deletion of the last role from a user
CREATE TRIGGER prevent_user_without_roles
BEFORE DELETE ON user_roles
FOR EACH ROW
BEGIN
    -- Check if this is the user's last role
    SELECT CASE 
        WHEN (SELECT COUNT(*) FROM user_roles WHERE user_id = OLD.user_id) = 1
        THEN RAISE(ABORT, 'Cannot remove the last role from a user. Users must have at least one role.')
    END;
END;

-- Create a view to easily check users without roles (for monitoring)
CREATE VIEW users_without_roles AS
SELECT u.id, u.email, u.company_id
FROM users u
LEFT JOIN user_roles ur ON u.id = ur.user_id
WHERE ur.user_id IS NULL;