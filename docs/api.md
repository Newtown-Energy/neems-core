# neems-api API Documentation

This guide covers all production API endpoints available in the
neems-api system, including authentication, user management, role
management, institution management, and utility endpoints.

If you are writing a front end that needs to talk to this backend, you
might refer to some sources of information:

 * this document
 * the `neems-api/src/api` directory
 * integration tests in `tests/*.rs` that exercise these endpoints.
 * the react codebase at `https://github.com/Newtown-Energy/neems-react`, which hits these endpoints
 * **TypeScript type definitions** generated from the Rust code (see "Generated TypeScript Types" section below)

You might point an LLM at those source to help you answer any question
you have or even to write code.

In addition to the endpoints listed here, there are a set of endpoints
that are only available in the `test-staging` feature.  Those are
documented in `api-testing.md`.  You shouldn't use those except for
testing purposes, as they shouldn't be enabled in production.

## Base URL

All API endpoints are prefixed with `/api/1/`

## JSON-Only API Responses

**Important:** This API is guaranteed to return JSON responses only. No HTML error pages will ever be served from `/api/*` routes.

### Error Response Format

All API errors return a standardized JSON structure:

```json
{
  "error": "Error message describing what went wrong",
  "status": 404,
  "path": "/api/1/endpoint"
}
```

### Framework-Level Error Handling

The API includes comprehensive error catchers for common HTTP status codes:

- **401 Unauthorized**: `{"error": "Unauthorized", "status": 401, "path": "/api/1/endpoint"}`
- **403 Forbidden**: `{"error": "Forbidden", "status": 403, "path": "/api/1/endpoint"}`
- **404 Not Found**: `{"error": "Not Found", "status": 404, "path": "/api/1/endpoint"}`
- **422 Unprocessable Entity**: `{"error": "Unprocessable Entity", "status": 422, "path": "/api/1/endpoint"}`
- **500 Internal Server Error**: `{"error": "Internal Server Error", "status": 500, "path": "/api/1/endpoint"}`

This ensures that frontend applications can always safely parse API responses as JSON without checking content types.

## Generated TypeScript Types

The API includes automatically generated TypeScript type definitions that match the Rust data structures exactly. These types are generated using the `ts-rs` crate and provide compile-time type safety for frontend development.

### Available Types

The generated types include:
- `ErrorResponse` - Standard error response structure used across all endpoints
- `LoginSuccessResponse` - Response structure for login and hello endpoints
- `CreateUserWithRolesRequest` - Request structure for creating users with roles
- `AddUserRoleRequest` - Request structure for adding roles to users
- `RemoveUserRoleRequest` - Request structure for removing roles from users
- `UpdateUserRequest` - Request structure for updating user information
- `CreateSiteRequest` - Request structure for creating sites
- `UpdateSiteRequest` - Request structure for updating sites
- All model types (`User`, `Role`, `Company`, `Site`, etc.)

### Using Generated Types

```typescript
import { LoginSuccessResponse, ErrorResponse, CreateUserWithRolesRequest } from './generated-types';

// Type-safe API calls
const loginResponse: LoginSuccessResponse = await fetch('/api/1/login', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ email: 'user@example.com', password: 'password' }),
  credentials: 'include'
}).then(res => res.json());

// Type-safe error handling
if (!response.ok) {
  const error: ErrorResponse = await response.json();
  console.error(error.error);
}
```

The types are automatically kept in sync with the backend code, ensuring that API changes are immediately reflected in the frontend type definitions.

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
- **Company:** `Newtown Energy`

**Note:** The environment variables (`NEEMS_DEFAULT_EMAIL` and
`NEEMS_DEFAULT_PASSWORD`) are only read during the initial admin user
creation. If an admin user already exists in the database, these
environment variables are ignored.

**Security Note:** Change these default credentials immediately in production environments.

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

## User Management

**Note:** All user endpoints now return user data with embedded role information
for improved efficiency. This eliminates the need for separate API calls to
fetch user roles in most cases. The separate role endpoints
(`/api/1/users/<user_id>/roles`) are deprecated but remain temporarily available
for backwards compatibility and specific role management operations.

### Create User

- **URL:** `/api/1/users`
- **Method:** `POST`
- **Purpose:** Creates a new user in the system with assigned roles
- **Authentication:** Required
- **Authorization:** Admin privileges required with company-based restrictions

#### Authorization Rules

- newtown-admin and newtown-staff can create users for any company
- Company admins can only create users for their own company
- Role assignment follows the same authorization rules as the "Add User Role" endpoint

#### Request Format

```json
{
  "email": "newuser@example.com",
  "password_hash": "hashed_password_string",
  "company_id": 1,
  "totp_secret": "optional_totp_secret",
  "role_names": ["admin", "staff"]
}
```

**Note:** At least one role must be provided in the `role_names` array.

#### Response

**Success (HTTP 201 Created):**
```json
{
  "id": 123,
  "email": "newuser@example.com",
  "password_hash": "hashed_password_string",
  "company_id": 1,
  "totp_secret": "optional_totp_secret",
  "created_at": "2023-01-01T00:00:00Z",
  "updated_at": "2023-01-01T00:00:00Z",
  "roles": [
    {
      "id": 1,
      "name": "admin",
      "description": "Administrator role"
    },
    {
      "id": 2,
      "name": "user",
      "description": "Basic user role"
    }
  ]
}
```

**Failure (HTTP 400 Bad Request):**
No roles provided in request

**Failure (HTTP 403 Forbidden):**
User doesn't have permission to create users for the specified company or assign specified roles

**Failure (HTTP 500 Internal Server Error):**
Database error or validation failure

### List Users

- **URL:** `/api/1/users`
- **Method:** `GET`
- **Purpose:** Retrieves users with their roles based on authorization level
- **Authentication:** Required
- **Authorization:** Company admins see users from their company; newtown-admin/staff see all users; regular users forbidden

#### Authorization Rules

- newtown-admin and newtown-staff can see all users
- Company admins can only see users from their own company
- Regular users cannot list users

#### Response

**Success (HTTP 200 OK):**
```json
[
  {
    "id": 1,
    "email": "user1@example.com",
    "password_hash": "hashed_password",
    "company_id": 1,
    "totp_secret": null,
    "created_at": "2023-01-01T00:00:00Z",
    "updated_at": "2023-01-01T00:00:00Z",
    "roles": [
      {
        "id": 1,
        "name": "admin",
        "description": "Administrator role"
      }
    ]
  },
  {
    "id": 2,
    "email": "user2@example.com",
    "password_hash": "hashed_password",
    "company_id": 2,
    "totp_secret": "secret",
    "created_at": "2023-01-01T00:00:00Z",
    "updated_at": "2023-01-01T00:00:00Z",
    "roles": [
      {
        "id": 2,
        "name": "user",
        "description": "Basic user role"
      },
      {
        "id": 3,
        "name": "staff",
        "description": "Staff role"
      }
    ]
  }
]
```

**Failure (HTTP 403 Forbidden):**
User doesn't have permission to list users

### Get User

- **URL:** `/api/1/users/<user_id>`
- **Method:** `GET`
- **Purpose:** Retrieves a specific user by ID with their roles
- **Authentication:** Required
- **Authorization:** Users can view their own profile; admins can view users based on company scope

#### Authorization Rules

- Users can always view their own profile
- newtown-admin and newtown-staff can view any user
- Company admins can only view users from their own company

#### Parameters

- `user_id` - The ID of the user to retrieve

#### Response

**Success (HTTP 200 OK):**
```json
{
  "id": 123,
  "email": "user@example.com",
  "password_hash": "hashed_password_string",
  "company_id": 1,
  "totp_secret": "optional_totp_secret",
  "created_at": "2023-01-01T00:00:00Z",
  "updated_at": "2023-01-01T00:00:00Z",
  "roles": [
    {
      "id": 1,
      "name": "admin",
      "description": "Administrator role"
    },
    {
      "id": 2,
      "name": "user",
      "description": "Basic user role"
    }
  ]
}
```

**Failure (HTTP 403 Forbidden):**
User doesn't have permission to view the specified user

**Failure (HTTP 404 Not Found):**
User with specified ID doesn't exist

### Update User

- **URL:** `/api/1/users/<user_id>`
- **Method:** `PUT`
- **Purpose:** Updates a user's information and returns the updated user with roles
- **Authentication:** Required
- **Authorization:** Users can update their own profile; admins can update users based on company scope

#### Authorization Rules

- Users can always update their own profile
- newtown-admin and newtown-staff can update any user
- Company admins can only update users from their own company

#### Parameters

- `user_id` - The ID of the user to update

#### Request Format

All fields are optional - only provided fields will be updated:

```json
{
  "email": "newemail@example.com",
  "password_hash": "new_hashed_password",
  "company_id": 2,
  "totp_secret": "new_totp_secret"
}
```

#### Response

**Success (HTTP 200 OK):**
```json
{
  "id": 123,
  "email": "newemail@example.com",
  "password_hash": "new_hashed_password",
  "company_id": 2,
  "totp_secret": "new_totp_secret",
  "created_at": "2023-01-01T00:00:00Z",
  "updated_at": "2023-01-01T12:30:00Z",
  "roles": [
    {
      "id": 1,
      "name": "admin",
      "description": "Administrator role"
    }
  ]
}
```

**Failure (HTTP 403 Forbidden):**
User doesn't have permission to update the specified user

**Failure (HTTP 404 Not Found):**
User with specified ID doesn't exist

### Delete User

- **URL:** `/api/1/users/<user_id>`
- **Method:** `DELETE`
- **Purpose:** Deletes a user from the system
- **Authentication:** Required
- **Authorization:** Admin privileges required with company-based restrictions

#### Authorization Rules

- newtown-admin and newtown-staff can delete any user
- Company admins can only delete users from their own company

**Warning:** This is a hard delete operation that cannot be undone and removes associated data like user roles and sessions.

#### Parameters

- `user_id` - The ID of the user to delete

#### Response

**Success (HTTP 204 No Content):**
No response body - user successfully deleted

**Failure (HTTP 403 Forbidden):**
User doesn't have permission to delete users

**Failure (HTTP 404 Not Found):**
User with specified ID doesn't exist

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

- **URL:** `/api/1/users/<user_id>/roles`
- **Method:** `POST`
- **Purpose:** Assigns a role to a user with authorization checks
- **Authentication:** Required (admin privileges with specific business rules)

#### Authorization Rules

1. `newtown-staff` and `newtown-admin` roles are reserved for Newtown Energy company
2. `newtown-admin` can set any user's role to anything
3. `newtown-staff` can set any user's role except `newtown-admin`
4. `admin` can set another user's role to `admin` if target user is at same company
5. Users must have at least one role (validated elsewhere)

#### Parameters

- `user_id` - The ID of the user to assign the role to

#### Request Format

```json
{
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
const response = await fetch('/api/1/users/123/roles', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    role_name: 'staff'
  }),
  credentials: 'include'
});
```

### Remove User Role

- **URL:** `/api/1/users/<user_id>/roles`
- **Method:** `DELETE`
- **Purpose:** Removes a role from a user with authorization checks
- **Authentication:** Required (same authorization rules as adding roles)

#### Authorization Rules

Same authorization rules as adding roles, plus:
- Users must retain at least one role after removal

#### Parameters

- `user_id` - The ID of the user to remove the role from

#### Request Format

```json
{
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
const response = await fetch('/api/1/users/123/roles', {
  method: 'DELETE',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
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
- **Authorization:** Only newtown-admin users can create roles

#### Authorization Rules

- Only users with 'newtown-admin' role can create new roles

#### Request Format

```json
{
  "name": "Administrator",
  "description": "Full system access"
}
```

#### Response

**Success (HTTP 201 Created):**
```json
{
  "id": 1,
  "name": "Administrator",
  "description": "Full system access"
}
```

**Failure (HTTP 403 Forbidden):**
```json
{
  "error": "Forbidden: only newtown-admin can create roles"
}
```

**Failure (HTTP 500 Internal Server Error):**
```json
{
  "error": "Internal server error while creating role"
}
```

### List Roles

- **URL:** `/api/1/roles`
- **Method:** `GET`
- **Purpose:** Retrieves all roles in the system
- **Authentication:** Required
- **Authorization:** All authenticated users can list roles

#### Response

**Success (HTTP 200 OK):**
```json
[
  {
    "id": 1,
    "name": "Administrator",
    "description": "Full system access"
  },
  {
    "id": 2,
    "name": "User",
    "description": "Basic user access"
  }
]
```

### Get Role

- **URL:** `/api/1/roles/<role_id>`
- **Method:** `GET`
- **Purpose:** Retrieves a specific role by ID
- **Authentication:** Required
- **Authorization:** All authenticated users can view individual roles

#### Parameters

- `role_id` - The ID of the role to retrieve

#### Response

**Success (HTTP 200 OK):**
```json
{
  "id": 1,
  "name": "Administrator",
  "description": "Full system access"
}
```

**Failure (HTTP 404 Not Found):**
```json
{
  "error": "Role with ID 123 not found"
}
```

**Failure (HTTP 500 Internal Server Error):**
```json
{
  "error": "Internal server error while getting role"
}
```

### Update Role

- **URL:** `/api/1/roles/<role_id>`
- **Method:** `PUT`
- **Purpose:** Updates a role's information
- **Authentication:** Required
- **Authorization:** Only newtown-admin users can update roles

#### Authorization Rules

- Only users with 'newtown-admin' role can update roles

#### Parameters

- `role_id` - The ID of the role to update

#### Request Format

All fields are optional - only provided fields will be updated:

```json
{
  "name": "Super Administrator",
  "description": "Updated description"
}
```

#### Response

**Success (HTTP 200 OK):**
```json
{
  "id": 1,
  "name": "Super Administrator",
  "description": "Updated description"
}
```

**Failure (HTTP 403 Forbidden):**
```json
{
  "error": "Forbidden: only newtown-admin can update roles"
}
```

**Failure (HTTP 404 Not Found):**
```json
{
  "error": "Role with ID 123 not found"
}
```

**Failure (HTTP 500 Internal Server Error):**
```json
{
  "error": "Internal server error while updating role"
}
```

### Delete Role

- **URL:** `/api/1/roles/<role_id>`
- **Method:** `DELETE`
- **Purpose:** Deletes a role from the system
- **Authentication:** Required
- **Authorization:** Only newtown-admin users can delete roles

#### Authorization Rules

- Only users with 'newtown-admin' role can delete roles

**Warning:** This operation will also remove the role from all users who have it assigned.

#### Parameters

- `role_id` - The ID of the role to delete

#### Response

**Success (HTTP 204 No Content):**
No response body - role successfully deleted

**Failure (HTTP 403 Forbidden):**
```json
{
  "error": "Forbidden: only newtown-admin can delete roles"
}
```

**Failure (HTTP 404 Not Found):**
```json
{
  "error": "Role with ID 123 not found"
}
```

**Failure (HTTP 500 Internal Server Error):**
```json
{
  "error": "Internal server error while deleting role"
}
```

## Site Management

### Create Site

- **URL:** `/api/1/sites`
- **Method:** `POST`
- **Purpose:** Creates a new site in the system
- **Authentication:** Required

#### Request Format

```json
{
  "name": "Main Office",
  "address": "123 Main St, City, State",
  "latitude": 40.7128,
  "longitude": -74.0060,
  "company_id": 1
}
```

#### Response

**Success (HTTP 201 Created):**
```json
{
  "id": 1,
  "name": "Main Office",
  "address": "123 Main St, City, State",
  "latitude": 40.7128,
  "longitude": -74.0060,
  "company_id": 1,
  "created_at": "2023-01-01T00:00:00Z",
  "updated_at": "2023-01-01T00:00:00Z"
}
```

**Failure (HTTP 500 Internal Server Error):**
Database error or validation failure

### Get Site

- **URL:** `/api/1/sites/<site_id>`
- **Method:** `GET`
- **Purpose:** Retrieves a specific site by ID
- **Authentication:** Required

#### Parameters

- `site_id` - The ID of the site to retrieve

#### Response

**Success (HTTP 200 OK):**
```json
{
  "id": 1,
  "name": "Main Office",
  "address": "123 Main St, City, State",
  "latitude": 40.7128,
  "longitude": -74.0060,
  "company_id": 1,
  "created_at": "2023-01-01T00:00:00Z",
  "updated_at": "2023-01-01T00:00:00Z"
}
```

**Failure (HTTP 404 Not Found):**
Site with specified ID doesn't exist

### List Sites

- **URL:** `/api/1/sites`
- **Method:** `GET`
- **Purpose:** Retrieves all sites in the system
- **Authentication:** Required

#### Response

**Success (HTTP 200 OK):**
```json
[
  {
    "id": 1,
    "name": "Main Office",
    "address": "123 Main St, City, State",
    "latitude": 40.7128,
    "longitude": -74.0060,
    "company_id": 1,
    "created_at": "2023-01-01T00:00:00Z",
    "updated_at": "2023-01-01T00:00:00Z"
  },
  {
    "id": 2,
    "name": "Branch Office",
    "address": "456 Oak Ave, City, State",
    "latitude": 40.7589,
    "longitude": -73.9851,
    "company_id": 1,
    "created_at": "2023-01-01T00:00:00Z",
    "updated_at": "2023-01-01T00:00:00Z"
  }
]
```

### Update Site

- **URL:** `/api/1/sites/<site_id>`
- **Method:** `PUT`
- **Purpose:** Updates a site's information
- **Authentication:** Required

#### Parameters

- `site_id` - The ID of the site to update

#### Request Format

All fields are optional - only provided fields will be updated:

```json
{
  "name": "Updated Office Name",
  "address": "456 New St, City, State",
  "latitude": 40.7589,
  "longitude": -73.9851
}
```

#### Response

**Success (HTTP 200 OK):**
```json
{
  "id": 1,
  "name": "Updated Office Name",
  "address": "456 New St, City, State",
  "latitude": 40.7589,
  "longitude": -73.9851,
  "company_id": 1,
  "created_at": "2023-01-01T00:00:00Z",
  "updated_at": "2023-01-01T12:30:00Z"
}
```

**Failure (HTTP 404 Not Found):**
Site with specified ID doesn't exist

### Delete Site

- **URL:** `/api/1/sites/<site_id>`
- **Method:** `DELETE`
- **Purpose:** Deletes a site from the system
- **Authentication:** Required

#### Parameters

- `site_id` - The ID of the site to delete

#### Response

**Success (HTTP 204 No Content):**
No response body - site successfully deleted

**Failure (HTTP 404 Not Found):**
Site with specified ID doesn't exist

## Company Management

### Create Company

- **URL:** `/api/1/companies`
- **Method:** `POST`
- **Purpose:** Creates a new company in the system
- **Authentication:** Required

#### Request Format

```json
{
  "name": "Example Corporation"
}
```

#### Response

**Success (HTTP 201 Created):**
```json
{
  "id": 1,
  "name": "Example Corporation",
  "created_at": "2023-01-01T00:00:00Z",
  "updated_at": "2023-01-01T00:00:00Z"
}
```

### List Companies

- **URL:** `/api/1/companies`
- **Method:** `GET`
- **Purpose:** Retrieves all companies in the system (ordered by ID)
- **Authentication:** Required

#### Response

**Success (HTTP 200 OK):**
```json
[
  {
    "id": 1,
    "name": "Example Corporation",
    "created_at": "2023-01-01T00:00:00Z",
    "updated_at": "2023-01-01T00:00:00Z"
  },
  {
    "id": 2,
    "name": "Another Company",
    "created_at": "2023-01-01T00:00:00Z",
    "updated_at": "2023-01-01T00:00:00Z"
  }
]
```

### List Company Sites

- **URL:** `/api/1/company/<company_id>/sites`
- **Method:** `GET`
- **Purpose:** Retrieves all sites for a specific company
- **Authentication:** Required
- **Authorization:** Users can see sites if they work for the company OR have newtown-admin/newtown-staff roles

#### Authorization Rules

- Users can see sites for their own company (same company_id)
- Users with 'newtown-admin' or 'newtown-staff' roles can see any company's sites

#### Parameters

- `company_id` - The ID of the company whose sites to retrieve

#### Response

**Success (HTTP 200 OK):**
```json
[
  {
    "id": 1,
    "name": "Main Office",
    "address": "123 Main St, City, State",
    "latitude": 40.7128,
    "longitude": -74.0060,
    "company_id": 1,
    "created_at": "2023-01-01T00:00:00Z",
    "updated_at": "2023-01-01T00:00:00Z"
  },
  {
    "id": 2,
    "name": "Branch Office",
    "address": "456 Oak Ave, City, State",
    "latitude": 40.7589,
    "longitude": -73.9851,
    "company_id": 1,
    "created_at": "2023-01-01T00:00:00Z",
    "updated_at": "2023-01-01T00:00:00Z"
  }
]
```

**Failure (HTTP 403 Forbidden):**
User doesn't have permission to view sites for this company

**Failure (HTTP 500 Internal Server Error):**
Database error during retrieval

#### Example

```js
const response = await fetch('/api/1/company/123/sites', {
  method: 'GET',
  credentials: 'include'
});
```

### List Company Users

- **URL:** `/api/1/company/<company_id>/users`
- **Method:** `GET`
- **Purpose:** Retrieves all users for a specific company with their roles
- **Authentication:** Required
- **Authorization:** Users can see users if they work for the company OR have newtown-admin/newtown-staff roles

#### Authorization Rules

- Users can see users for their own company (same company_id)
- Users with 'newtown-admin' or 'newtown-staff' roles can see any company's users

#### Parameters

- `company_id` - The ID of the company whose users to retrieve

#### Response

**Success (HTTP 200 OK):**
```json
[
  {
    "id": 1,
    "email": "user1@company.com",
    "password_hash": "hashed_password",
    "company_id": 1,
    "totp_secret": null,
    "created_at": "2023-01-01T00:00:00Z",
    "updated_at": "2023-01-01T00:00:00Z",
    "roles": [
      {
        "id": 1,
        "name": "admin",
        "description": "Administrator role"
      }
    ]
  },
  {
    "id": 2,
    "email": "user2@company.com",
    "password_hash": "hashed_password",
    "company_id": 1,
    "totp_secret": "secret",
    "created_at": "2023-01-01T00:00:00Z",
    "updated_at": "2023-01-01T00:00:00Z",
    "roles": [
      {
        "id": 2,
        "name": "user",
        "description": "Basic user role"
      },
      {
        "id": 3,
        "name": "staff",
        "description": "Staff role"
      }
    ]
  }
]
```

**Failure (HTTP 403 Forbidden):**
User doesn't have permission to view users for this company

**Failure (HTTP 500 Internal Server Error):**
Database error during retrieval

#### Example

```js
const response = await fetch('/api/1/company/123/users', {
  method: 'GET',
  credentials: 'include'
});
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

All error responses follow these standardized formats:

**Application-specific errors:**
```json
{
  "error": "Error description"
}
```

**Framework-level errors:**
```json
{
  "error": "Error message",
  "status": 404,
  "path": "/api/1/endpoint"
}
```

## Best Practices

1. **Always use HTTPS** in production
2. **Include `credentials: 'include'`** in all authenticated requests
3. **Always parse responses as JSON** - the API guarantees JSON-only responses
4. **Handle errors gracefully** with user-friendly messages from the `error` field
5. **Never try to access the session cookie** from JavaScript
6. **Check authentication status** using the `/api/1/hello` endpoint
7. **Use generated TypeScript types** for compile-time type safety

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

