# Role Management Endpoints

This document covers all role management API endpoints in the neems-api system.

See [api.md](api.md) for general information about the API including base URL, error handling, and authentication.

## Role Management

### OData Features

The Roles endpoint supports OData v4 features:

- **Query Options**: Use `$select`, `$filter`, `$orderby`, `$top`, `$skip`, `$count`, and `$expand`
- **Collection Response Format**: Results are wrapped in OData envelope with `@odata.context`, `@odata.count`, and `value` properties

### Create Role

- **URL:** `/api/1/Roles`
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

- **URL:** `/api/1/Roles`
- **Method:** `GET`
- **Purpose:** Retrieves all roles in the system
- **Authentication:** Required
- **Authorization:** All authenticated users can list roles

#### Response

**Success (HTTP 200 OK):**
```json
{
  "@odata.context": "http://localhost/api/1/$metadata#Roles",
  "@odata.count": 2,
  "value": [
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
}
```

### Get Role

- **URL:** `/api/1/Roles/<role_id>`
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

- **URL:** `/api/1/Roles/<role_id>`
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

- **URL:** `/api/1/Roles/<role_id>`
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

## Role System Overview

### Default Roles

The system includes several built-in roles with specific permissions:

- **newtown-admin**: Full system administrator with unrestricted access
- **newtown-staff**: Newtown Energy staff with cross-company access
- **admin**: Company administrator with access to their company's data
- **staff**: Company staff member with standard access
- **user**: Basic user with limited access

### Role Authorization Rules

1. **Newtown-specific roles** (`newtown-admin`, `newtown-staff`) are reserved for Newtown Energy company users
2. **Role management** (create, update, delete) is restricted to `newtown-admin` users
3. **Role viewing** is available to all authenticated users
4. **Role assignment** follows hierarchical rules based on the assigner's role level

See [api-users.md](api-users.md) for role assignment rules when adding/removing roles from users.