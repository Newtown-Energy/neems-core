-- Add database-level trigger to enforce Newtown roles restriction
-- Business rule: newtown-staff and newtown-admin roles are reserved for Newtown Energy institution users

CREATE TRIGGER enforce_newtown_roles_restriction
BEFORE INSERT ON user_roles
FOR EACH ROW
BEGIN
    -- Check if the role being assigned is a Newtown-specific role
    -- and if the user is not from Newtown Energy institution
    SELECT CASE
        WHEN NEW.role_id IN (
            SELECT id FROM roles WHERE name IN ('newtown-admin', 'newtown-staff')
        ) AND NEW.user_id NOT IN (
            SELECT u.id FROM users u 
            JOIN institutions i ON u.institution_id = i.id 
            WHERE i.name = 'Newtown Energy'
        )
        THEN RAISE(ABORT, 'Newtown roles (newtown-admin, newtown-staff) can only be assigned to Newtown Energy institution users')
    END;
END;

-- Also create a trigger for UPDATE operations in case role_id or user_id is modified
CREATE TRIGGER enforce_newtown_roles_restriction_update
BEFORE UPDATE ON user_roles
FOR EACH ROW
BEGIN
    -- Check if the role being assigned is a Newtown-specific role
    -- and if the user is not from Newtown Energy institution
    SELECT CASE
        WHEN NEW.role_id IN (
            SELECT id FROM roles WHERE name IN ('newtown-admin', 'newtown-staff')
        ) AND NEW.user_id NOT IN (
            SELECT u.id FROM users u 
            JOIN institutions i ON u.institution_id = i.id 
            WHERE i.name = 'Newtown Energy'
        )
        THEN RAISE(ABORT, 'Newtown roles (newtown-admin, newtown-staff) can only be assigned to Newtown Energy institution users')
    END;
END;