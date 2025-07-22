# Role System Documentation

This document describes the role-based access control (RBAC) system in neems-core for frontend developers integrating with the backend API.

## Overview

The neems-core system uses a hierarchical role system with four distinct roles that control user permissions and access to various system features. Roles are tightly integrated with companies and enforce specific business rules around authorization.

## The Four Roles

### 1. `newtown-admin`
- **Purpose:** System administrator for Newtown Energy
- **Privilege Level:** Highest - full system control
- **Company Restriction:** **Newtown Energy only**
- **Key Capabilities:**
  - Can assign/remove any role to/from any user
  - Access to all system features and endpoints
  - Can manage users across all companies
  - Full administrative privileges

### 2. `newtown-staff`
- **Purpose:** Staff member for Newtown Energy
- **Privilege Level:** High - broad system access
- **Company Restriction:** **Newtown Energy only**
- **Key Capabilities:**
  - Can assign/remove most roles to/from any user
  - **Cannot** assign `newtown-admin` role (privilege escalation protection)
  - Access to most administrative features
  - Can manage users across all companies (with role limitations)

### 3. `admin`
- **Purpose:** Administrator for individual companies
- **Privilege Level:** Medium - company-scoped control
- **Company Restriction:** Can only manage users within their own company
- **Key Capabilities:**
  - Can assign/remove `admin` role to users in the **same company only**
  - Cannot assign Newtown-specific roles
  - Company-local administrative access
  - Limited to managing their own company's users

### 4. `staff`
- **Purpose:** Standard system user
- **Privilege Level:** Basic - standard user access
- **Company Restriction:** None specific
- **Key Capabilities:**
  - Basic system access
  - Can view their own profile and roles
  - No role assignment privileges
  - Standard user features only

## Role Hierarchy

```
newtown-admin     (Full system control)
    ↓
newtown-staff     (System-wide management, limited by elevation rules)
    ↓
admin             (Company-scoped administration)
    ↓
staff             (Basic access)
```

## Business Rules

### Company-Based Restrictions

#### Newtown Energy Exclusive Roles
- `newtown-admin` and `newtown-staff` roles are **strictly reserved** for users belonging to "Newtown Energy" company
- This restriction is enforced at both API and database levels
- **Even `newtown-admin` cannot assign Newtown roles to non-Newtown users**

#### Cross-Company Management
- `newtown-admin` and `newtown-staff` can manage users from any company
- `admin` users can only manage users within their own company
- `staff` role has no management capabilities

### Role Assignment Authorization Matrix

| Your Role | Can Assign To Users | Restrictions |
|-----------|-------------------|--------------|
| `newtown-admin` | **Any role** | None - full privileges |
| `newtown-staff` | Any role **except** `newtown-admin` | Cannot create other newtown-admins |
| `admin` | `admin` role only | Only to users in **same company** |
| `staff` | None | No assignment privileges |

### Role Removal Rules
- Same authorization rules apply as assignment
- **Critical:** Users must always have at least one role
- Attempting to remove the last role from a user will fail
- Frontend should prevent UI actions that would violate this rule

### User Role Requirements
- Every user **must have at least one role** at all times
- New users automatically receive `staff` role if no other role is assigned
- The system prevents creating "roleless" users

## Frontend Integration Guidelines

### Checking User Permissions

When building frontend interfaces, use these patterns to check permissions:

#### Getting Current User's Roles
```js
// Get authenticated user's roles
const response = await fetch('/api/1/hello', { credentials: 'include' });
if (response.ok) {
  const userInfo = await response.text(); // "Hello, user@example.com!"
  // Then get roles for the authenticated user
  const rolesResponse = await fetch(`/api/1/users/${userId}/roles`, {
    credentials: 'include'
  });
  const roles = await rolesResponse.json();
}
```

#### Permission-Based UI Rendering
```js
// Example: Show admin panel only to appropriate roles
function canAccessAdminPanel(userRoles) {
  return userRoles.some(role =>
    ['newtown-admin', 'newtown-staff', 'admin'].includes(role.name)
  );
}

// Example: Show role management for users who can assign roles
function canManageRoles(userRoles, targetUserCompany, currentUserCompany) {
  const hasNewtownAdmin = userRoles.some(r => r.name === 'newtown-admin');
  const hasNewtownStaff = userRoles.some(r => r.name === 'newtown-staff');
  const hasAdmin = userRoles.some(r => r.name === 'admin');

  if (hasNewtownAdmin) return true; // Can manage anyone
  if (hasNewtownStaff) return true; // Can manage anyone (with role restrictions)
  if (hasAdmin) return targetUserCompany === currentUserCompany; // Same company only
  return false;
}
```

### Role Assignment UI Patterns

#### Dropdown/Selection Logic
```js
function getAssignableRoles(userRoles, targetUser, currentUser) {
  const allRoles = ['staff', 'admin', 'newtown-staff', 'newtown-admin'];

  if (userRoles.some(r => r.name === 'newtown-admin')) {
    return allRoles; // Can assign any role
  }

  if (userRoles.some(r => r.name === 'newtown-staff')) {
    return allRoles.filter(role => role !== 'newtown-admin'); // Cannot assign newtown-admin
  }

  if (userRoles.some(r => r.name === 'admin')) {
    // Can only assign admin to users in same company
    if (targetUser.company_id === currentUser.company_id) {
      return ['admin'];
    }
  }

  return []; // No assignment privileges
}
```

#### Newtown Role Validation
```js
function canAssignNewtownRole(targetUser, roleName) {
  if (['newtown-admin', 'newtown-staff'].includes(roleName)) {
    // Must be Newtown Energy company user
    return targetUser.company_name === 'Newtown Energy';
  }
  return true;
}
```

### Error Handling

#### Common API Responses
```js
// Role assignment/removal responses
const response = await fetch('/api/1/users/roles', {
  method: 'POST', // or DELETE
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ user_id: 123, role_name: 'admin' }),
  credentials: 'include'
});

if (!response.ok) {
  switch (response.status) {
    case 400:
      alert('Cannot remove user\'s last role'); // Only for DELETE
      break;
    case 403:
      alert('You don\'t have permission to assign/remove this role');
      break;
    case 500:
      alert('Server error - please try again');
      break;
  }
}
```

### UI Recommendations

#### Role Display
- Always show roles with clear visual hierarchy
- Use color coding: Newtown roles (special), admin (elevated), staff (standard)
- Display company context when relevant

#### Role Management Interface
- Prevent users from removing their own critical roles
- Show warnings before removing roles (especially admin roles)
- Disable options that would violate business rules
- Provide clear feedback about permission limitations

#### Company Context
- Always display user's company in role management interfaces
- Clearly indicate when Newtown-exclusive roles are involved
- Show company-based restrictions in tooltips or help text

## API Endpoints Summary

The role system integrates with numerous API endpoints. Key role-related endpoints include:

**Role Management:**
- `GET /api/1/users/<user_id>/roles` - Get user's roles
- `POST /api/1/users/<user_id>/roles` - Add role to user
- `DELETE /api/1/users/<user_id>/roles` - Remove role from user
- `GET /api/1/roles` - List all available roles
- `POST /api/1/roles` - Create new role (newtown-admin only)
- `PUT /api/1/roles/<role_id>` - Update role (newtown-admin only)
- `DELETE /api/1/roles/<role_id>` - Delete role (newtown-admin only)

**User Management:**
- `GET /api/1/users` - List users (authorization-scoped)
- `POST /api/1/users` - Create user (admin privileges required)
- `GET /api/1/users/<user_id>` - Get user details
- `PUT /api/1/users/<user_id>` - Update user
- `DELETE /api/1/users/<user_id>` - Delete user (admin privileges required)

**Company Management:**
- `GET /api/1/companies` - List companies
- `POST /api/1/companies` - Create company
- `GET /api/1/company/<company_id>/users` - List company users
- `GET /api/1/company/<company_id>/sites` - List company sites

**Authentication & Session:**
- `POST /api/1/login` - Login and establish session
- `GET /api/1/hello` - Check authentication status and get user info
- `POST /api/1/logout` - End session

**Other Endpoints:**
- `GET /api/1/sites` - List/manage sites
- `POST /api/1/sites` - Create sites
- `GET /api/1/status` - Health check
- `GET /api/1/fixphrase/encode/<lat>/<lon>` - FixPhrase encoding

See `@docs/api.md` for complete endpoint documentation including detailed request/response formats, authorization rules, and examples.

## Security Considerations

- Role checks should be implemented on both frontend (UX) and backend (security)
- Frontend role checks are for user experience only - **never rely on them for security**
- Always validate permissions server-side before performing sensitive operations
- The backend enforces all business rules with database-level constraints as defense-in-depth

## Testing Your Role Integration

The system includes test endpoints (when `test-staging` feature is enabled) that demonstrate role-based access:

- `/api/1/test/admin-only` - Requires `admin` role
- `/api/1/test/staff-only` - Requires `staff` role
- `/api/1/test/newtown-admin-only` - Requires `newtown-admin` role
- `/api/1/test/newtown-staff-only` - Requires `newtown-staff` role

Use these endpoints to verify your frontend role detection and permission logic works correctly.