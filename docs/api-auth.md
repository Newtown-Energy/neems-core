# Authentication Endpoints

This document covers all authentication-related API endpoints in the neems-api system.

See [api.md](api.md) for general information about the API including base URL, error handling, and session management.

## Authentication Endpoints

**Important:** The login and hello endpoints return identical data structures for the same user to ensure API consistency. Both endpoints return the same JSON object containing `user_id`, `email`, `institution_name`, and `roles` fields.

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
  "company_name": "Example Corp",
  "roles": ["staff", "admin"]
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

**Success (HTTP 200 OK):**
```json
{
  "message": "Logout successful",
  "status": "ok"
}
```

#### Example

```js
const response = await fetch('/api/1/logout', {
  method: 'POST',
  credentials: 'include'
});

const data = await response.json();
console.log(data.message); // "Logout successful"
```

### Hello (Authentication Check)

- **URL:** `/api/1/hello`
- **Method:** `GET`
- **Purpose:** Returns user information for authenticated users; useful for checking authentication status
- **Authentication:** Required

**Note:** This endpoint returns exactly the same data structure as the login endpoint for consistency.

#### Response

**Success (HTTP 200 OK):**
```json
{
  "user_id": 123,
  "email": "user@example.com", 
  "company_name": "Example Corp",
  "roles": ["staff", "admin"]
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

## Default Admin Credentials

The system automatically creates a default admin user on first startup **only if no admin user already exists** in the database. The default credentials are:

- **Email:** `admin@example.com` (configurable via `NEEMS_DEFAULT_EMAIL` environment variable)
- **Password:** `admin` (configurable via `NEEMS_DEFAULT_PASSWORD` environment variable)
- **Role:** `newtown-admin`
- **Company:** `Newtown Energy`

**Note:** The environment variables (`NEEMS_DEFAULT_EMAIL` and `NEEMS_DEFAULT_PASSWORD`) are only read during the initial admin user creation. If an admin user already exists in the database, these environment variables are ignored.

**Security Note:** Change these default credentials immediately in production environments.

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
  const response = await fetch('/api/1/logout', {
    method: 'POST',
    credentials: 'include'
  });
  
  const data = await response.json();
  console.log(data.message); // "Logout successful"
  return data;
}
```