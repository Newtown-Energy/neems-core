# neems-api API Documentation

This is the master documentation index for all API endpoints in the neems-api system. The API documentation has been organized into focused documents for better maintainability and navigation.

## Quick Navigation

### Core API Documents
- **[Authentication](api-auth.md)** - Login, logout, session management
- **[User Management](api-users.md)** - User CRUD operations and role assignment
- **[Role Management](api-roles.md)** - Role creation and management
- **[Company Management](api-companies.md)** - Company operations and listings
- **[Site Management](api-sites.md)** - Physical location management
- **[Data Access](api-data.md)** - Sensor readings and data sources
- **[Utilities](api-utilities.md)** - Health checks and location services
- **[Testing Endpoints](api-testing.md)** - Test-staging feature endpoints

### Additional Resources

If you are writing a front end that needs to talk to this backend, you
might refer to some sources of information:

 * The documents linked above
 * the `neems-api/src/api` directory
 * integration tests in `tests/*.rs` that exercise these endpoints.
 * the react codebase at `https://github.com/Newtown-Energy/neems-react`, which hits these endpoints
 * **TypeScript type definitions** generated from the Rust code (see "Generated TypeScript Types" section below)

You might point an LLM at those source to help you answer any question
you have or even to write code.

In addition to the endpoints in the main documents above, there are a set of endpoints
that are only available in the `test-staging` feature.  Those are
documented in [api-testing.md](api-testing.md).  You shouldn't use those except for
testing purposes, as they shouldn't be enabled in production.

## Base URL

All API endpoints are prefixed with `/api/1/`

## JSON-Only API Responses

**Important:** This API should return JSON responses only. No HTML error pages should ever be served from `/api/*` routes.  If the API returns non-JSON, that is a bug.

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

For detailed authentication information, see [Authentication](api-auth.md).

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

For specific endpoint documentation, see the individual documents linked at the top of this page.

## Summary

The neems-api provides a comprehensive REST API for managing users, companies, sites, roles, and data access. The API is fully documented across multiple focused documents for easy navigation and maintenance. All endpoints return JSON responses and support robust error handling and authentication through session cookies.
