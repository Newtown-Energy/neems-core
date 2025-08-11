-- Drop all the triggers we created

-- Drop sessions triggers
DROP TRIGGER IF EXISTS sessions_delete_log;
DROP TRIGGER IF EXISTS sessions_update_log;
DROP TRIGGER IF EXISTS sessions_insert_log;

-- Drop sites triggers
DROP TRIGGER IF EXISTS sites_delete_log;
DROP TRIGGER IF EXISTS sites_update_log;
DROP TRIGGER IF EXISTS sites_insert_log;

-- Drop companies triggers
DROP TRIGGER IF EXISTS companies_delete_log;
DROP TRIGGER IF EXISTS companies_update_log;
DROP TRIGGER IF EXISTS companies_insert_log;