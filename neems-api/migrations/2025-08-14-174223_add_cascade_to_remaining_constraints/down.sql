-- Recreate tables without CASCADE constraints (original version)

-- Recreate sites table without CASCADE
DROP TABLE sites;
CREATE TABLE sites (
    id INTEGER PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    address TEXT NOT NULL,
    latitude DOUBLE PRECISION NOT NULL,
    longitude DOUBLE PRECISION NOT NULL,
    company_id INTEGER NOT NULL REFERENCES companies(id),
    UNIQUE (company_id, name)
);

-- Recreate user_roles table without CASCADE
DROP TABLE user_roles;
CREATE TABLE user_roles (
    user_id INTEGER NOT NULL REFERENCES users(id),
    role_id INTEGER NOT NULL REFERENCES roles(id),
    PRIMARY KEY (user_id, role_id)
);
