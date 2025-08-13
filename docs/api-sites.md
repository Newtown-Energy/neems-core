# Site Management Endpoints

This document covers all site management API endpoints in the neems-api system.

See [api.md](api.md) for general information about the API including base URL, error handling, and authentication.

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

## Site System Overview

### Site Properties

Sites represent physical locations associated with companies:

- **name**: Display name for the site
- **address**: Physical address as a text string
- **latitude/longitude**: Geographic coordinates for mapping
- **company_id**: The company that owns this site

### Relationship to Other Entities

- Sites belong to companies
- Data sources can be associated with sites
- Sites provide geographic context for data collection

### Access Control

Site access is typically controlled through company membership:
- Users can access sites belonging to their company
- Newtown staff/admin can access all sites across companies

See [api-companies.md](api-companies.md) for endpoints to list sites by company.