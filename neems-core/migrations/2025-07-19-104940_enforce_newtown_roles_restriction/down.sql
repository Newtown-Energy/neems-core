-- Remove the triggers that enforce Newtown roles restriction

DROP TRIGGER IF EXISTS enforce_newtown_roles_restriction;
DROP TRIGGER IF EXISTS enforce_newtown_roles_restriction_update;