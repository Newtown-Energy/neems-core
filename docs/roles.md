# Role System Documentation

This document describes the role-based access control (RBAC) system in neems-core for frontend developers integrating with the backend API.

## Overview

The neems-core system uses a hierarchical role system with four distinct roles that control user permissions and access to various system features. Roles are tightly integrated with institutions and enforce specific business rules around authorization.

## The Four Roles

### 1. `newtown-admin`
- **Purpose:** System administrator for Newtown Energy
- **Privilege Level:** Highest - full system control
- **Institution Restriction:** **Newtown Energy only**
- **Key Capabilities:**
  - Can assign/remove any role to/from any user
  - Access to all system features and endpoints
  - Can manage users across all institutions
  - Full administrative privileges

### 2. `newtown-staff`  
- **Purpose:** Staff member for Newtown Energy
- **Privilege Level:** High - broad system access
- **Institution Restriction:** **Newtown Energy only**
- **Key Capabilities:**
  - Can assign/remove most roles to/from any user
  - **Cannot** assign `newtown-admin` role (privilege escalation protection)
  - Access to most administrative features
  - Can manage users across all institutions (with role limitations)

### 3. `admin`
- **Purpose:** Administrator for individual institutions
- **Privilege Level:** Medium - institution-scoped control  
- **Institution Restriction:** Can only manage users within their own institution
- **Key Capabilities:**
  - Can assign/remove `admin` role to users in the **same institution only**
  - Cannot assign Newtown-specific roles
  - Institution-local administrative access
  - Limited to managing their own institution's users

### 4. `user`
- **Purpose:** Standard system user
- **Privilege Level:** Basic - standard user access
- **Institution Restriction:** None specific
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
admin             (Institution-scoped administration)
    ↓
user              (Basic access)
```

## Business Rules

### Institution-Based Restrictions

#### Newtown Energy Exclusive Roles
- `newtown-admin` and `newtown-staff` roles are **strictly reserved** for users belonging to "Newtown Energy" institution
- This restriction is enforced at both API and database levels
- **Even `newtown-admin` cannot assign Newtown roles to non-Newtown users**

#### Cross-Institution Management
- `newtown-admin` and `newtown-staff` can manage users from any institution
- `admin` users can only manage users within their own institution
- `user` role has no management capabilities

### Role Assignment Authorization Matrix

| Your Role | Can Assign To Users | Restrictions |
|-----------|-------------------|--------------|
| `newtown-admin` | **Any role** | None - full privileges |
| `newtown-staff` | Any role **except** `newtown-admin` | Cannot create other newtown-admins |
| `admin` | `admin` role only | Only to users in **same institution** |
| `user` | None | No assignment privileges |

### Role Removal Rules
- Same authorization rules apply as assignment
- **Critical:** Users must always have at least one role
- Attempting to remove the last role from a user will fail
- Frontend should prevent UI actions that would violate this rule

### User Role Requirements
- Every user **must have at least one role** at all times
- New users automatically receive `user` role if no other role is assigned
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
function canManageRoles(userRoles, targetUserInstitution, currentUserInstitution) {
  const hasNewtownAdmin = userRoles.some(r => r.name === 'newtown-admin');
  const hasNewtownStaff = userRoles.some(r => r.name === 'newtown-staff');
  const hasAdmin = userRoles.some(r => r.name === 'admin');
  
  if (hasNewtownAdmin) return true; // Can manage anyone
  if (hasNewtownStaff) return true; // Can manage anyone (with role restrictions)
  if (hasAdmin) return targetUserInstitution === currentUserInstitution; // Same institution only
  return false;
}
```

### Role Assignment UI Patterns

#### Dropdown/Selection Logic
```js
function getAssignableRoles(userRoles, targetUser, currentUser) {
  const allRoles = ['user', 'admin', 'newtown-staff', 'newtown-admin'];
  
  if (userRoles.some(r => r.name === 'newtown-admin')) {
    return allRoles; // Can assign any role
  }
  
  if (userRoles.some(r => r.name === 'newtown-staff')) {
    return allRoles.filter(role => role !== 'newtown-admin'); // Cannot assign newtown-admin
  }
  
  if (userRoles.some(r => r.name === 'admin')) {
    // Can only assign admin to users in same institution
    if (targetUser.institution_id === currentUser.institution_id) {
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
    // Must be Newtown Energy institution user
    return targetUser.institution_name === 'Newtown Energy';
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
- Use color coding: Newtown roles (special), admin (elevated), user (standard)
- Display institution context when relevant

#### Role Management Interface
- Prevent users from removing their own critical roles
- Show warnings before removing roles (especially admin roles)
- Disable options that would violate business rules
- Provide clear feedback about permission limitations

#### Institution Context
- Always display user's institution in role management interfaces  
- Clearly indicate when Newtown-exclusive roles are involved
- Show institution-based restrictions in tooltips or help text

## API Endpoints Summary

- `GET /api/1/users/<user_id>/roles` - Get user's roles
- `POST /api/1/users/roles` - Add role to user  
- `DELETE /api/1/users/roles` - Remove role from user
- `GET /api/1/roles` - List all available roles

See `api.md` for complete endpoint documentation including request/response formats.

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