-- Demo data: Insert two companies (idempotent)
INSERT OR IGNORE INTO companies (name, created_at, updated_at) VALUES
  ('Sunny Solar', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP),
  ('Best BESS', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP);

-- Demo users for Sunny Solar
-- Admin password: 'admin', User password: 'password'
INSERT OR IGNORE INTO users (email, password_hash, created_at, updated_at, company_id, totp_secret) VALUES
  ('admin@sunnysolar.com', '$argon2id$v=19$m=524288,t=2,p=1$c2FsdHNhbHQ$NdqozkTTiU1EOLdmtz9leb0ArHMLVX3kL+yIvpudyGM', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, (SELECT id FROM companies WHERE name = 'Sunny Solar'), ''),
  ('user@sunnysolar.com', '$argon2id$v=19$m=524288,t=2,p=1$c2FsdHNhbHQ$ZsS3YPKw1dvAygqWSQqDnxbS7S/a0Wj0Zxz5QJ2N8gg', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, (SELECT id FROM companies WHERE name = 'Sunny Solar'), '');

-- Demo users for Best BESS
-- Admin password: 'admin', User password: 'password'
INSERT OR IGNORE INTO users (email, password_hash, created_at, updated_at, company_id, totp_secret) VALUES
  ('admin@bestbess.com', '$argon2id$v=19$m=524288,t=2,p=1$c2FsdHNhbHQ$NdqozkTTiU1EOLdmtz9leb0ArHMLVX3kL+yIvpudyGM', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, (SELECT id FROM companies WHERE name = 'Best BESS'), ''),
  ('user@bestbess.com', '$argon2id$v=19$m=524288,t=2,p=1$c2FsdHNhbHQ$ZsS3YPKw1dvAygqWSQqDnxbS7S/a0Wj0Zxz5QJ2N8gg', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, (SELECT id FROM companies WHERE name = 'Best BESS'), '');

-- Assign admin role to admin users (role_id 3 is 'admin')
INSERT OR IGNORE INTO user_roles (user_id, role_id) VALUES
  ((SELECT id FROM users WHERE email = 'admin@sunnysolar.com'), 3),
  ((SELECT id FROM users WHERE email = 'admin@bestbess.com'), 3);

-- Assign user role to regular users (role_id 4 is 'user')
INSERT OR IGNORE INTO user_roles (user_id, role_id) VALUES
  ((SELECT id FROM users WHERE email = 'user@sunnysolar.com'), 4),
  ((SELECT id FROM users WHERE email = 'user@bestbess.com'), 4);
