# neems-api test-staging API Documentation

This guide covers all `test-staging` feature API endpoints available
in the neems-api system.

See `api.md` for information on auth, base URL, and the rest of the API.

## Test and Staging Endpoints

**Note:** These endpoints are only available when the `test-staging` feature is enabled during compilation.

### Admin-Only Test Endpoint

- **URL:** `/api/1/test/admin-only`
- **Method:** `GET`
- **Purpose:** Test endpoint demonstrating admin role authorization
- **Authentication:** Required (admin role)

#### Response

**Success (HTTP 200 OK):**
```json
{
  "message": "Admin access granted to user@example.com",
  "endpoint": "admin-only",
  "required_role": "admin"
}
```

**Failure (HTTP 401/403):** Authorization failure

### Staff-Only Test Endpoint

- **URL:** `/api/1/test/staff-only`
- **Method:** `GET`
- **Purpose:** Test endpoint demonstrating staff role authorization
- **Authentication:** Required (staff role)

#### Response

**Success (HTTP 200 OK):**
```json
{
  "message": "Staff access granted to user@example.com",
  "endpoint": "staff-only",
  "required_role": "staff"
}
```

### Newtown Admin-Only Test Endpoint

- **URL:** `/api/1/test/newtown-admin-only`
- **Method:** `GET`
- **Purpose:** Test endpoint demonstrating newtown-admin role authorization
- **Authentication:** Required (newtown-admin role)

#### Response

**Success (HTTP 200 OK):**
```json
{
  "message": "Newtown admin access granted to user@example.com",
  "endpoint": "newtown-admin-only",
  "required_role": "newtown-admin"
}
```

### Newtown Staff-Only Test Endpoint

- **URL:** `/api/1/test/newtown-staff-only`
- **Method:** `GET`
- **Purpose:** Test endpoint demonstrating newtown-staff role authorization
- **Authentication:** Required (newtown-staff role)

#### Response

**Success (HTTP 200 OK):**
```json
{
  "message": "Newtown staff access granted to user@example.com",
  "endpoint": "newtown-staff-only",
  "required_role": "newtown-staff"
}
```

### Multi-Role Test Endpoint

- **URL:** `/api/1/test/admin-and-staff`
- **Method:** `GET`
- **Purpose:** Test endpoint requiring both admin AND staff roles
- **Authentication:** Required (both admin and staff roles)

#### Response

**Success (HTTP 200 OK):**
```json
{
  "message": "Multi-role access granted to user@example.com",
  "endpoint": "admin-and-staff",
  "required_roles": ["admin", "staff"],
  "user_roles": ["admin", "staff"]
}
```

**Failure (HTTP 403 Forbidden):** Missing required roles

### No-Admin Test Endpoint

- **URL:** `/api/1/test/no-admin-allowed`
- **Method:** `GET`
- **Purpose:** Test endpoint that forbids admin role access
- **Authentication:** Required (any role except admin)

#### Response

**Success (HTTP 200 OK):**
```json
{
  "message": "Non-admin access granted to user@example.com",
  "endpoint": "no-admin-allowed",
  "forbidden_roles": ["admin"],
  "user_roles": ["staff"]
}
```

**Failure (HTTP 403 Forbidden):** User has admin role

### Flexible Role Test Endpoint

- **URL:** `/api/1/test/any-admin-or-staff`
- **Method:** `GET`
- **Purpose:** Test endpoint accepting any of several roles
- **Authentication:** Required (admin OR staff OR newtown-admin role)

#### Response

**Success (HTTP 200 OK):**
```json
{
  "message": "Flexible role access granted to user@example.com",
  "endpoint": "any-admin-or-staff",
  "accepted_roles": ["admin", "staff", "newtown-admin"],
  "user_roles": ["admin"]
}
```

**Failure (HTTP 403 Forbidden):** Missing any accepted roles
