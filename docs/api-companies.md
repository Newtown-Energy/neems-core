# Company Management Endpoints

This document covers all company management API endpoints in the neems-api system.

See [api.md](api.md) for general information about the API including base URL, error handling, and authentication.

## Company Management

### OData Features

The Companies endpoint supports OData v4 features:

- **Query Options**: Use `$select`, `$filter`, `$orderby`, `$top`, `$skip`, `$count`, and `$expand`
- **Collection Response Format**: Results are wrapped in OData envelope with `@odata.context`, `@odata.count`, and `value` properties
- **Navigation Properties**: Access related data via `/api/1/Companies/{id}/Users` or `/api/1/Companies/{id}/Sites`, or use `$expand=Users` or `$expand=Sites`

### Create Company

- **URL:** `/api/1/Companies`
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

- **URL:** `/api/1/Companies`
- **Method:** `GET`
- **Purpose:** Retrieves all companies in the system (ordered by ID)
- **Authentication:** Required

#### Response

**Success (HTTP 200 OK):**
```json
{
  "@odata.context": "http://localhost/api/1/$metadata#Companies",
  "@odata.count": 2,
  "value": [
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
}
```

### Delete Company

- **URL:** `/api/1/Companies/<company_id>`
- **Method:** `DELETE`
- **Purpose:** Deletes a company from the system
- **Authentication:** Required
- **Authorization:** Admin privileges required

**Warning:** This operation permanently deletes the company and may affect related data.

#### Parameters

- `company_id` - The ID of the company to delete

#### Response

**Success (HTTP 204 No Content):**
No response body - company successfully deleted

**Failure (HTTP 404 Not Found):**
Company with specified ID doesn't exist

**Failure (HTTP 500 Internal Server Error):**
Database error during deletion

### List Company Sites

- **URL:** `/api/1/Companies/<company_id>/Sites`
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
const response = await fetch('/api/1/Companies/123/Sites', {
  method: 'GET',
  credentials: 'include'
});
```

### List Company Users

- **URL:** `/api/1/Companies/<company_id>/Users`
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
const response = await fetch('/api/1/Companies/123/Users', {
  method: 'GET',
  credentials: 'include'
});
```

## Company System Overview

### Company Hierarchy

Companies serve as the primary organizational unit in the system:

- Each user belongs to exactly one company
- Sites are owned by companies
- Data sources can be associated with companies
- Access control is largely based on company membership

### Special Companies

- **Newtown Energy**: The system operator company whose users can have special `newtown-admin` and `newtown-staff` roles with cross-company access

### Authorization Model

- **Regular users**: Can only access data from their own company
- **Company admins**: Can manage users and resources within their company
- **Newtown staff/admin**: Can access and manage resources across all companies