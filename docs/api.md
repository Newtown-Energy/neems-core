# neems-core API Documentation

This guide covers all production API endpoints available in the
neems-core system, including authentication, user management, role
management, institution management, and utility endpoints.

If you are writing a front end that needs to talk to this backend, you
might refer to some sources of information:

 * this document
 * the `neems-core/src/api` directory
 * integration tests in `tests/*.rs` that exercise these endpoints.
 * the react codebase at `https://github.com/Newtown-Energy/neems-react`, which hits these endpoints

You might point an LLM at those source to help you answer any question
you have or even to write code.

In addition to the endpoints listed here, there are a set of endpoints
that are only available in the `test-staging` feature.  Those are
documented in `api-testing.md`.  You shouldn't use those except for
testing purposes, as they shouldn't be enabled in production.

## Base URL

All API endpoints are prefixed with `/api/1/`

## Authentication

Most endpoints require authentication via session cookies. Use the
login endpoint to obtain a session cookie, then include `credentials:
'include'` in all subsequent requests.

### Default Admin Credentials

The system automatically creates a default admin user on first startup
**only if no admin user already exists** in the database. The default
credentials are:

- **Email:** `admin@example.com` (configurable via `NEEMS_DEFAULT_EMAIL` environment variable)
- **Password:** `admin` (configurable via `NEEMS_DEFAULT_PASSWORD` environment variable)
- **Role:** `newtown-admin`
- **Institution:** `Newtown Energy`

**Note:** The environment variables (`NEEMS_DEFAULT_EMAIL` and
`NEEMS_DEFAULT_PASSWORD`) are only read during the initial admin user
creation. If an admin user already exists in the database, these
environment variables are ignored.

**Security Note:** Change these default credentials immediately in production environments.

## Authentication Endpoints

### Login

- **URL:** `/api/1/login`
- **Method:** `POST`
- **Purpose:** Authenticates a user by email and password, and sets a secure session cookie
- **Authentication:** None required

#### Request Format

```json
{
  "email": "user@example.com",
  "password": "userpassword"
}
```

#### Response

**Success (HTTP 200 OK):**
```json
{
  "user_id": 123,
  "email": "user@example.com",
  "institution_name": "Example Corp",
  "roles": ["user", "admin"]
}
```
- Also sets session cookie named `session` (HTTP-only, secure, SameSite=Lax)

**Failure (HTTP 401 Unauthorized):**
```json
{ "error": "Invalid credentials" }
```

#### Example

```js
const response = await fetch('/api/1/login', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    email: 'testuser@example.com',
    password: 'testpassword'
  }),
  credentials: 'include'
});
```

### Logout

- **URL:** `/api/1/logout`
- **Method:** `POST`
- **Purpose:** Terminates the current session and removes the session cookie
- **Authentication:** None required (works with or without valid session)

#### Response

**Always returns HTTP 200 OK** - Success regardless of session state

#### Example

```js
const response = await fetch('/api/1/logout', {
  method: 'POST',
  credentials: 'include'
});
```

### Hello (Authentication Check)

- **URL:** `/api/1/hello`
- **Method:** `GET`
- **Purpose:** Returns a greeting for authenticated users; useful for checking authentication status
- **Authentication:** Required

#### Response

**Success (HTTP 200 OK):**
```json
{
  "user_id": 123,
  "email": "user@example.com", 
  "institution_name": "Example Corp",
  "roles": ["user", "admin"]
}
```

**Failure (HTTP 401 Unauthorized):**
Session invalid or expired

#### Example

```js
const response = await fetch('/api/1/hello', {
  method: 'GET',
  credentials: 'include'
});
```

## User Management

### Create User

- **URL:** `/api/1/users`
- **Method:** `POST`
- **Purpose:** Creates a new user in the system
- **Authentication:** Required

#### Request Format

```json
{
  "email": "newuser@example.com",
  "password_hash": "hashed_password_string",
  "institution_id": 1,
  "totp_secret": "optional_totp_secret"
}
```

#### Response

**Success (HTTP 201 Created):**
```json
{
  "id": 123,
  "email": "newuser@example.com",
  "password_hash": "hashed_password_string",
  "institution_id": 1,
  "totp_secret": "optional_totp_secret",
  "created_at": "2023-01-01T00:00:00Z",
  "updated_at": "2023-01-01T00:00:00Z"
}
```

**Failure (HTTP 500 Internal Server Error):**
Database error or validation failure

### List Users

- **URL:** `/api/1/users`
- **Method:** `GET`
- **Purpose:** Retrieves all users in the system
- **Authentication:** Required

#### Response

**Success (HTTP 200 OK):**
```json
[
  {
    "id": 1,
    "email": "user1@example.com",
    "password_hash": "hashed_password",
    "institution_id": 1,
    "totp_secret": null,
    "created_at": "2023-01-01T00:00:00Z",
    "updated_at": "2023-01-01T00:00:00Z"
  },
  {
    "id": 2,
    "email": "user2@example.com",
    "password_hash": "hashed_password",
    "institution_id": 2,
    "totp_secret": "secret",
    "created_at": "2023-01-01T00:00:00Z",
    "updated_at": "2023-01-01T00:00:00Z"
  }
]
```

### Get User Roles

- **URL:** `/api/1/users/<user_id>/roles`
- **Method:** `GET`
- **Purpose:** Retrieves all roles assigned to a specific user
- **Authentication:** Required (users can view their own roles, or users with admin privileges can view any user's roles)

#### Parameters

- `user_id` - The ID of the user whose roles to retrieve

#### Response

**Success (HTTP 200 OK):**
```json
[
  {
    "id": 1,
    "name": "admin",
    "description": "Administrator role",
    "created_at": "2023-01-01T00:00:00Z",
    "updated_at": "2023-01-01T00:00:00Z"
  },
  {
    "id": 2,
    "name": "staff",
    "description": "Staff role",
    "created_at": "2023-01-01T00:00:00Z",
    "updated_at": "2023-01-01T00:00:00Z"
  }
]
```

**Failure (HTTP 403 Forbidden):**
User doesn't have permission to view the specified user's roles

#### Example

```js
const response = await fetch('/api/1/users/123/roles', {
  method: 'GET',
  credentials: 'include'
});
```

### Add User Role

- **URL:** `/api/1/users/roles`
- **Method:** `POST`
- **Purpose:** Assigns a role to a user with authorization checks
- **Authentication:** Required (admin privileges with specific business rules)

#### Authorization Rules

1. `newtown-staff` and `newtown-admin` roles are reserved for Newtown Energy institution
2. `newtown-admin` can set any user's role to anything
3. `newtown-staff` can set any user's role except `newtown-admin`
4. `admin` can set another user's role to `admin` if target user is at same institution
5. Users must have at least one role (validated elsewhere)

#### Request Format

```json
{
  "user_id": 123,
  "role_name": "staff"
}
```

#### Response

**Success (HTTP 200 OK):**
No response body - role successfully assigned

**Failure (HTTP 403 Forbidden):**
User doesn't have permission to assign the specified role

**Failure (HTTP 500 Internal Server Error):**
Database error or validation failure

#### Example

```js
const response = await fetch('/api/1/users/roles', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    user_id: 123,
    role_name: 'staff'
  }),
  credentials: 'include'
});
```

### Remove User Role

- **URL:** `/api/1/users/roles`
- **Method:** `DELETE`
- **Purpose:** Removes a role from a user with authorization checks
- **Authentication:** Required (same authorization rules as adding roles)

#### Authorization Rules

Same authorization rules as adding roles, plus:
- Users must retain at least one role after removal

#### Request Format

```json
{
  "user_id": 123,
  "role_name": "staff"
}
```

#### Response

**Success (HTTP 200 OK):**
No response body - role successfully removed

**Failure (HTTP 400 Bad Request):**
User would have no roles remaining after removal

**Failure (HTTP 403 Forbidden):**
User doesn't have permission to remove the specified role

**Failure (HTTP 500 Internal Server Error):**
Database error or validation failure

#### Example

```js
const response = await fetch('/api/1/users/roles', {
  method: 'DELETE',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    user_id: 123,
    role_name: 'staff'
  }),
  credentials: 'include'
});
```

## Role Management

### Create Role

- **URL:** `/api/1/roles`
- **Method:** `POST`
- **Purpose:** Creates a new role in the system
- **Authentication:** Required

#### Request Format

```json
{
  "name": "Administrator",
  "description": "Full system access"
}
```

#### Response

**Success (HTTP 200 OK):**
```json
{
  "id": 1,
  "name": "Administrator",
  "description": "Full system access",
  "created_at": "2023-01-01T00:00:00Z",
  "updated_at": "2023-01-01T00:00:00Z"
}
```

### List Roles

- **URL:** `/api/1/roles`
- **Method:** `GET`
- **Purpose:** Retrieves all roles in the system
- **Authentication:** Required

#### Response

**Success (HTTP 200 OK):**
```json
[
  {
    "id": 1,
    "name": "Administrator",
    "description": "Full system access",
    "created_at": "2023-01-01T00:00:00Z",
    "updated_at": "2023-01-01T00:00:00Z"
  },
  {
    "id": 2,
    "name": "User",
    "description": "Basic user access",
    "created_at": "2023-01-01T00:00:00Z",
    "updated_at": "2023-01-01T00:00:00Z"
  }
]
```

## Institution Management

### Create Institution

- **URL:** `/api/1/institutions`
- **Method:** `POST`
- **Purpose:** Creates a new institution in the system
- **Authentication:** Required

#### Request Format

```json
{
  "name": "Example University"
}
```

#### Response

**Success (HTTP 201 Created):**
```json
{
  "id": 1,
  "name": "Example University",
  "created_at": "2023-01-01T00:00:00Z",
  "updated_at": "2023-01-01T00:00:00Z"
}
```

### List Institutions

- **URL:** `/api/1/institutions`
- **Method:** `GET`
- **Purpose:** Retrieves all institutions in the system (ordered by ID)
- **Authentication:** Required

#### Response

**Success (HTTP 200 OK):**
```json
[
  {
    "id": 1,
    "name": "Example University",
    "created_at": "2023-01-01T00:00:00Z",
    "updated_at": "2023-01-01T00:00:00Z"
  },
  {
    "id": 2,
    "name": "Another Institution",
    "created_at": "2023-01-01T00:00:00Z",
    "updated_at": "2023-01-01T00:00:00Z"
  }
]
```

## Utility Endpoints

### Health Status

- **URL:** `/api/1/status`
- **Method:** `GET`
- **Purpose:** Returns the health status of the application
- **Authentication:** None required

#### Response

**Success (HTTP 200 OK):**
```json
{
  "status": "running"
}
```

### FixPhrase Encoding

- **URL:** `/api/1/fixphrase/encode/<lat>/<lon>`
- **Method:** `GET`
- **Purpose:** Encodes latitude/longitude coordinates into a FixPhrase string
- **Authentication:** None required

#### Parameters

- `lat` - Latitude coordinate (between -90 and 90)
- `lon` - Longitude coordinate (between -180 and 180)

#### Response

**Success (HTTP 200 OK):**
```json
{
  "phrase": "example.fixphrase.string",
  "latitude": 40.7128,
  "longitude": -74.0060,
  "accuracy": 10.0
}
```

**Failure (HTTP 400 Bad Request):**
```json
{
  "error": "Invalid coordinates"
}
```

#### Example

```js
const response = await fetch('/api/1/fixphrase/encode/40.7128/-74.0060');
```

## Session Management

### Cookie Properties

All authenticated endpoints use session cookies with the following properties:
- **Name:** `session`
- **HTTP-only:** Cannot be accessed via JavaScript
- **Secure:** Only sent over HTTPS
- **SameSite=Lax:** Helps prevent CSRF attacks

### Making Authenticated Requests

Always include `credentials: 'include'` in your fetch requests:

```js
const response = await fetch('/api/1/users', {
  method: 'GET',
  credentials: 'include'
});
```

## Error Handling

### Common HTTP Status Codes

- **200 OK:** Request successful
- **201 Created:** Resource created successfully
- **400 Bad Request:** Invalid request data
- **401 Unauthorized:** Authentication required or invalid
- **500 Internal Server Error:** Server error

### Error Response Format

Most error responses follow this format:

```json
{
  "error": "Error description"
}
```

## Best Practices

1. **Always use HTTPS** in production
2. **Include `credentials: 'include'`** in all authenticated requests
3. **Handle errors gracefully** with user-friendly messages
4. **Never try to access the session cookie** from JavaScript
5. **Check authentication status** using the `/api/1/hello` endpoint

## Quick Examples

### React Login Function

```js
async function login(email, password) {
  const res = await fetch('/api/1/login', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    credentials: 'include',
    body: JSON.stringify({ email, password })
  });
  if (res.ok) {
    return true;
  } else {
    const err = await res.json();
    throw new Error(err.error);
  }
}
```

### Check Authentication Status

```js
async function isAuthenticated() {
  const res = await fetch('/api/1/hello', {
    credentials: 'include'
  });
  return res.ok;
}
```

### Logout Function

```js
async function logout() {
  await fetch('/api/1/logout', {
    method: 'POST',
    credentials: 'include'
  });
}
```

