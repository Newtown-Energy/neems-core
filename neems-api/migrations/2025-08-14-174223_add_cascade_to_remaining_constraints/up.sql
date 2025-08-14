-- Add CASCADE behavior to remaining foreign key constraints

-- Recreate sites table with CASCADE delete for company_id
DROP TABLE sites;
CREATE TABLE sites (
    id INTEGER PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    address TEXT NOT NULL,
    latitude DOUBLE PRECISION NOT NULL,
    longitude DOUBLE PRECISION NOT NULL,
    company_id INTEGER NOT NULL,
    UNIQUE (company_id, name),
    FOREIGN KEY(company_id) REFERENCES companies(id) ON DELETE CASCADE
);

-- Recreate user_roles table with CASCADE delete for user_id
DROP TABLE user_roles;
CREATE TABLE user_roles (
    user_id INTEGER NOT NULL,
    role_id INTEGER NOT NULL,
    PRIMARY KEY (user_id, role_id),
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY(role_id) REFERENCES roles(id)
);

-- Recreate the prevent_user_without_roles trigger that was lost when table was dropped
CREATE TRIGGER prevent_user_without_roles
BEFORE DELETE ON user_roles
FOR EACH ROW
BEGIN
    SELECT CASE
        WHEN (SELECT COUNT(*) FROM user_roles WHERE user_id = OLD.user_id) = 1
        THEN RAISE(ABORT, 'Cannot remove the last role from a user. Users must have at least one role.')
    END;
END;

-- Recreate the Newtown roles restriction triggers that were lost when table was dropped
CREATE TRIGGER enforce_newtown_roles_restriction
BEFORE INSERT ON user_roles
FOR EACH ROW
BEGIN
    -- Check if the role being assigned is a Newtown-specific role
    -- and if the user is not from Newtown Energy company
    SELECT CASE
        WHEN NEW.role_id IN (
            SELECT id FROM roles WHERE name IN ('newtown-admin', 'newtown-staff')
        ) AND NEW.user_id NOT IN (
            SELECT u.id FROM users u 
            JOIN companies i ON u.company_id = i.id 
            WHERE i.name = 'Newtown Energy'
        )
        THEN RAISE(ABORT, 'Newtown roles (newtown-admin, newtown-staff) can only be assigned to Newtown Energy company users')
    END;
END;

CREATE TRIGGER enforce_newtown_roles_restriction_update
BEFORE UPDATE ON user_roles
FOR EACH ROW
BEGIN
    -- Check if the role being assigned is a Newtown-specific role
    -- and if the user is not from Newtown Energy company
    SELECT CASE
        WHEN NEW.role_id IN (
            SELECT id FROM roles WHERE name IN ('newtown-admin', 'newtown-staff')
        ) AND NEW.user_id NOT IN (
            SELECT u.id FROM users u 
            JOIN companies i ON u.company_id = i.id 
            WHERE i.name = 'Newtown Energy'
        )
        THEN RAISE(ABORT, 'Newtown roles (newtown-admin, newtown-staff) can only be assigned to Newtown Energy company users')
    END;
END;

-- Recreate sites triggers that were lost when table was dropped
CREATE TRIGGER sites_insert_log 
AFTER INSERT ON sites
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('sites', NEW.id, 'create', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER sites_update_log 
AFTER UPDATE ON sites
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('sites', NEW.id, 'update', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER sites_delete_log 
AFTER DELETE ON sites
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('sites', OLD.id, 'delete', CURRENT_TIMESTAMP);
END;
