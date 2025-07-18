# Plan

Make session guards aware of roles.

╭────────────────────────────────────────────────────────────────────────────────────────────────────────╮
│ Ready to code?                                                                                         │
│                                                                                                        │
│ Here is Claude's plan:                                                                                 │
│ ╭────────────────────────────────────────────────────────────────────────────────────────────────────╮ │
│ │ Complete Plan: Role-Based Authentication System                                                    │ │
│ │                                                                                                    │ │
│ │ 1. Database-Level Constraint                                                                       │ │
│ │                                                                                                    │ │
│ │ Add a database constraint to ensure every user has at least one role:                              │ │
│ │ - Create a new migration that adds a CHECK constraint to enforce this rule                         │ │
│ │ - Assign default roles to any existing users without roles before adding the constraint            │ │
│ │                                                                                                    │ │
│ │ 2. Update AuthenticatedUser Structure                                                              │ │
│ │                                                                                                    │ │
│ │ - Change role: Role to roles: Vec<Role> in AuthenticatedUser                                       │ │
│ │ - Update the database query to load ALL roles for the user (using .load::<Role>() instead of       │ │
│ │ .first::<Role>())                                                                                  │ │
│ │ - Handle the case where users have multiple roles                                                  │ │
│ │                                                                                                    │ │
│ │ 3. Create Role-Based Authorization Guards                                                          │ │
│ │                                                                                                    │ │
│ │ Implement specialized guards for role-based access control:                                        │ │
│ │ - RequireAnyRole(Vec<String>) - User must have at least one of the specified roles                 │ │
│ │ - RequireAllRoles(Vec<String>) - User must have all of the specified roles                         │ │
│ │ - RequireNoRoles(Vec<String>) - User must not have any of the specified roles                      │ │
│ │ - RequireExactRoles(Vec<String>) - User must have exactly these roles (no more, no less)           │ │
│ │                                                                                                    │ │
│ │ These guards will:                                                                                 │ │
│ │ - Accept an empty list for no role restrictions                                                    │ │
│ │ - Use AuthenticatedUser internally to get the user's roles                                         │ │
│ │ - Compare the user's roles against the required roles                                              │ │
│ │ - Return appropriate HTTP status codes (403 Forbidden for insufficient roles, 401 Unauthorized for │ │
│ │  unauthenticated)                                                                                  │ │
│ │                                                                                                    │ │
│ │ 4. ORM Functions                                                                                   │ │
│ │                                                                                                    │ │
│ │ Create user-role management functions:                                                             │ │
│ │ - assign_user_role(conn, user_id, role_id) - Assign a role to a user                               │ │
│ │ - remove_user_role(conn, user_id, role_id) - Remove a role from a user                             │ │
│ │ - get_user_roles(conn, user_id) - Get all roles for a user                                         │ │
│ │ - user_has_role(conn, user_id, role_name) - Check if user has a specific role                      │ │
│ │                                                                                                    │ │
│ │ 5. Test Updates                                                                                    │ │
│ │                                                                                                    │ │
│ │ - Update add_dummy_data to assign a default role to test users                                     │ │
│ │ - Add tests for the new role-based guards                                                          │ │
│ │ - Update existing tests to handle the new roles: Vec<Role> field                                   │ │
│ │                                                                                                    │ │
│ │ 6. Migration Strategy                                                                              │ │
│ │                                                                                                    │ │
│ │ Create a new migration that:                                                                       │ │
│ │ - Assigns a default role (user) to any existing users without roles                                │ │
│ │ - Adds the database constraint to prevent users without roles                                      │ │
│ │ - Updates the schema accordingly                                                                   │ │
│ │                                                                                                    │ │
│ │ 7. Usage Examples                                                                                  │ │
│ │                                                                                                    │ │
│ │ #[get("/admin-only")]                                                                              │ │
│ │ fn admin_only(user: RequireAnyRole(vec!["admin".to_string()])) -> String {                         │ │
│ │     format!("Welcome admin: {}", user.user.email)                                                  │ │
│ │ }                                                                                                  │ │
│ │                                                                                                    │ │
│ │ #[get("/staff-or-admin")]                                                                          │ │
│ │ fn staff_or_admin(user: RequireAnyRole(vec!["staff".to_string(), "admin".to_string()])) -> String  │ │
│ │ {                                                                                                  │ │
│ │     format!("Welcome: {}", user.user.email)                                                        │ │
│ │ }                                                                                                  │ │
│ │                                                                                                    │ │
│ │ #[get("/public-but-authenticated")]                                                                │ │
│ │ fn public_auth(user: RequireAnyRole(vec![])) -> String {                                           │ │
│ │     format!("Any authenticated user: {}", user.user.email)                                         │ │
│ │ }                                                                                                  │ │
│ │                                                                                                    │ │
│ │ Implementation Order:                                                                              │ │
│ │ 1. Create user-role assignment ORM functions                                                       │ │
│ │ 2. Create migration with constraint                                                                │ │
│ │ 3. Update AuthenticatedUser to handle Vec                                                          │ │
│ │ 4. Create role-based authorization guards                                                          │ │
│ │ 5. Update test setup                                                                               │ │
│ │ 6. Add comprehensive tests                                                                         │ │
│ │ 7. Update documentation and examples                                                               │ │
│ ╰────────────────────────────────────────────────────────────────────────────────────────────────────╯ │

# PROGRESS
## 1. Create user-role assignment ORM functions
DONE

## 2. Create migration with constraint

## 3. Update AuthenticatedUser to handle Vec
DONE
## 4. Create role-based authorization guards
## 5. Update test setup
## 6. Add comprehensive tests
## 7. Update documentation and examples
