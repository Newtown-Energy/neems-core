-- Insert initial roles into the roles table

INSERT INTO roles (name, description) VALUES
  ('newtown-admin', 'Administrator for Newtown'),
  ('newtown-staff', 'Staff member for Newtown'),
  ('admin', 'Administrator for Site Owner'),
  ('user', 'User');

INSERT INTO companies (name, created_at, updated_at)
VALUES ('Newtown Energy', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

