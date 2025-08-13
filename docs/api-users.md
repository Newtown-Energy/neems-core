# User Management Endpoints

This document covers all user management API endpoints in the neems-api system.

See [api.md](api.md) for general information about the API including base URL, error handling, and authentication.

## User Management

**Note:** All user endpoints now return user data with embedded role information for improved efficiency. This eliminates the need for separate API calls to fetch user roles in most cases. The separate role endpoints (`/api/1/users/<user_id>/roles`) are deprecated but remain temporarily available for backwards compatibility and specific role management operations.

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

## User Role Management

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