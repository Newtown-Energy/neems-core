# NEEMS Admin CLI

A command-line interface for administrative management of NEEMS database operations. This tool provides comprehensive database management capabilities for users, companies, sites, roles, and system operations.

## Overview

`neems-admin` is a Rust-based CLI utility that interfaces directly with the NEEMS SQLite database through the `neems-api` ORM layer. It's designed for system administrators and developers who need to perform database operations outside of the main API interface.  You do not need a running neems-api to use neems-admin.  This code should not act via the API.

## Key Features

- **User Management**: Create, list, edit, remove users, and manage passwords
- **Company Management**: Create, list, edit, remove companies with cascading deletes
- **Site Management**: Create, list, edit, remove sites
- **Role Management**: Create, list, and manage user roles
- **Search Functionality**: Flexible search with regex and fixed-string support
- **Security**: Secure password prompting without echo
- **Data Integrity**: Cascading deletes and referential integrity checks
- **Interactive Confirmations**: Safety prompts for destructive operations

## Installation & Setup

### Prerequisites

- Rust toolchain (see workspace Cargo.toml for version requirements)
- SQLite database with NEEMS schema
- Environment variables configured (see Configuration section)

### Building

```bash
# From the workspace root
cargo build -p neems-admin

# Or build with optimizations
cargo build --release -p neems-admin
```

### Configuration

The CLI reads database configuration from environment variables or `.env` file:

```bash
# Database connection
DATABASE_URL=path/to/your/neems.db

# Optional: Set logging level
RUST_LOG=info
```

## Usage

### Basic Commands

```bash
# Show all available commands
neems-admin --help

# Show help for a specific command
neems-admin user --help
neems-admin company --help
neems-admin site --help
neems-admin role --help
```

### User Management

```bash
# List all users
neems-admin user ls

# Search users with regex
neems-admin user ls "admin@.*"

# Search users with fixed string
neems-admin user ls -F admin@example.com

# Create a new user
neems-admin user add -e user@example.com -c 1

# Create user with password (will prompt securely)
neems-admin user add -e user@example.com -p mypassword -c 1

# Change user password
neems-admin user change-password -e user@example.com

# Edit user details
neems-admin user edit --id 1 --email newemail@example.com

# Add role to user
neems-admin user add-role -e user@example.com -r admin

# Remove role from user  
neems-admin user rm-role -e user@example.com -r user

# Set all roles for user (replaces existing)
neems-admin user set-roles -e user@example.com -r "admin,user"

# Remove users matching pattern
neems-admin user rm "test.*@example.com"
```

### Company Management

```bash
# List all companies
neems-admin company ls

# Search companies
neems-admin company ls "Solar"

# Create a new company
neems-admin company add --name "Solar Energy Corp"

# Edit company
neems-admin company edit --id 1 --name "New Company Name"

# Remove companies (with cascade delete confirmation)
neems-admin company rm "Test.*"
```

### Site Management

```bash
# List all sites
neems-admin site ls

# List sites for specific company
neems-admin site ls -c 1

# Create a new site
neems-admin site add --name "Main Office" --address "123 Main St" --latitude 40.7128 --longitude -74.0060 --company-id 1

# Edit site
neems-admin site edit --id 1 --name "Updated Site Name"

# Remove sites
neems-admin site rm "Old.*"
```

### Role Management

```bash
# List all roles
neems-admin role ls

# Create a new role
neems-admin role add --name "operator" --description "Site operator role"
```

## Architecture

### Database Integration

`neems-admin` integrates with the NEEMS database through several key components:

- **ORM Layer**: Uses `neems-api` ORM functions for all database operations
- **Entity Activity Tracking**: Supports the centralized `entity_activity` table for timestamps
- **Referential Integrity**: Maintains foreign key relationships and cascading deletes

### Schema Compatibility

The CLI is compatible with the latest NEEMS schema featuring:

- **Centralized Timestamps**: Uses `entity_activity` table instead of per-table timestamp columns
- **Trigger-based Updates**: Database triggers automatically handle creation/update timestamps
- **Backward Compatibility**: Gracefully handles missing timestamp data (shows "Unknown")

### Command Structure

The CLI is organized into modules:

```
src/
├── main.rs                           # CLI entry point and command routing
└── admin_cli/
    ├── company_commands.rs          # Company management operations
    ├── user_commands.rs             # User management operations  
    ├── site_commands.rs             # Site management operations
    ├── role_commands.rs             # Role management operations
    └── utils.rs                     # Shared utilities (DB connection, etc.)
```

### Key Implementation Details

1. **Password Security**: Uses `argon2` for password hashing and `rpassword` for secure input
2. **Search Flexibility**: Supports both regex patterns and fixed-string matching
3. **Error Handling**: Comprehensive error handling with user-friendly messages  
4. **Transaction Safety**: Database operations are wrapped in appropriate transactions
5. **Confirmation Prompts**: Interactive prompts prevent accidental data loss

## Testing

The CLI includes comprehensive tests covering all major functionality:

```bash
# Run all tests
cargo test -p neems-admin

# Run tests with output
cargo test -p neems-admin -- --nocapture

# Run specific test
cargo test -p neems-admin test_user_add_impl
```

### Test Coverage

- User CRUD operations and role management
- Company CRUD operations with cascade deletes  
- Site CRUD operations
- Password management and hashing
- Search functionality (regex and fixed-string)
- Error handling and edge cases
- Schema compatibility with entity activity tracking

## Security Considerations

### Password Handling

- Passwords are never stored in plaintext
- `argon2` used for secure password hashing
- Interactive password prompts don't echo to terminal
- Password validation prevents weak passwords

### Database Access

- Direct database access requires appropriate file permissions
- No network exposure - local database access only
- Comprehensive input validation prevents SQL injection
- Cascading delete confirmations prevent accidental data loss

### Best Practices

1. **Backup First**: Always backup database before bulk operations
2. **Test Queries**: Use `ls` commands to preview what `rm` commands will affect
3. **Environment Security**: Protect `.env` files containing database paths
4. **Access Control**: Restrict file system access to authorized users only

## Troubleshooting

### Common Issues

**Database Connection Errors**
```bash
# Check database file permissions
ls -la /path/to/database.db

# Verify DATABASE_URL environment variable
echo $DATABASE_URL
```

**Permission Denied**
- Ensure user has read/write access to database file and directory
- Check SQLite file locks from other processes

**Migration Errors**
- Database schema may need to be updated via the main API application
- Check for pending migrations in `neems-api`

### Debug Mode

Enable verbose logging:

```bash
RUST_LOG=debug neems-admin user ls
```

## Contributing

When modifying `neems-admin`:

1. **ORM Consistency**: Always use `neems-api` ORM functions rather than direct SQL
2. **Test Coverage**: Add tests for new functionality
3. **Error Handling**: Provide clear, actionable error messages
4. **Documentation**: Update help text and this README for new features
5. **Schema Compatibility**: Ensure compatibility with entity activity tracking system

## Related Components

- **neems-api**: Provides ORM layer and database schema
- **neems-data**: Data collection and processing system
- **Database Schema**: SQLite database with triggers for activity tracking

---

For detailed command-specific help, use `neems-admin <command> --help`.
