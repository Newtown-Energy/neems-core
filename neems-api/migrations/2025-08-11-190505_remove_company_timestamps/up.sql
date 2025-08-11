-- Remove timestamp columns from companies table
-- Need to handle all foreign keys and triggers carefully

-- Temporarily drop ALL triggers that might reference companies
DROP TRIGGER IF EXISTS companies_delete_log;
DROP TRIGGER IF EXISTS companies_update_log; 
DROP TRIGGER IF EXISTS companies_insert_log;
DROP TRIGGER IF EXISTS enforce_newtown_roles_restriction;
DROP TRIGGER IF EXISTS enforce_newtown_roles_restriction_update;
DROP TRIGGER IF EXISTS prevent_user_without_roles;

-- Disable foreign key constraints temporarily
PRAGMA foreign_keys = OFF;

-- Create new companies table without timestamp columns
CREATE TABLE companies_new (
    id INTEGER PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE
);

-- Copy existing data (excluding timestamp columns)
INSERT INTO companies_new (id, name)
SELECT id, name FROM companies;

-- Drop old table and rename new one
DROP TABLE companies;
ALTER TABLE companies_new RENAME TO companies;

-- Re-enable foreign key constraints
PRAGMA foreign_keys = ON;

-- Recreate company triggers for the new table
CREATE TRIGGER companies_insert_log 
AFTER INSERT ON companies
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('companies', NEW.id, 'create', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER companies_update_log 
AFTER UPDATE ON companies
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('companies', NEW.id, 'update', CURRENT_TIMESTAMP);
END;

CREATE TRIGGER companies_delete_log 
AFTER DELETE ON companies
FOR EACH ROW
BEGIN 
    INSERT INTO entity_activity (table_name, entity_id, operation_type, timestamp) 
    VALUES ('companies', OLD.id, 'delete', CURRENT_TIMESTAMP);
END;

-- Recreate other triggers that were dropped
CREATE TRIGGER IF NOT EXISTS enforce_newtown_roles_restriction
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
            JOIN companies c ON u.company_id = c.id 
            WHERE c.name = 'Newtown Energy'
        )
        THEN RAISE(ABORT, 'Newtown roles (newtown-admin, newtown-staff) can only be assigned to Newtown Energy company users')
    END;
END;

CREATE TRIGGER IF NOT EXISTS enforce_newtown_roles_restriction_update
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
            JOIN companies c ON u.company_id = c.id 
            WHERE c.name = 'Newtown Energy'
        )
        THEN RAISE(ABORT, 'Newtown roles (newtown-admin, newtown-staff) can only be assigned to Newtown Energy company users')
    END;
END;

CREATE TRIGGER IF NOT EXISTS prevent_user_without_roles
BEFORE DELETE ON user_roles
FOR EACH ROW
BEGIN
    SELECT CASE
        WHEN (SELECT COUNT(*) FROM user_roles WHERE user_id = OLD.user_id) = 1
        THEN RAISE(ABORT, 'Cannot remove the last role from a user. Users must have at least one role.')
    END;
END;