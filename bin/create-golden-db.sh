#!/bin/bash

# Golden database creation script for NEEMS using neems-admin CLI
# This script replaces the Rust binary create-golden-db and creates the same golden database
# that can be copied for fast test execution.

set -e  # Exit on any error

echo "Creating golden database for NEEMS testing..."

# Check if we have a recent golden database (within the last hour)
TARGET_DIR="$(pwd)/target"
RECENT_DB=""

if [ -d "$TARGET_DIR" ]; then
    # Find the most recent golden database (created within the last hour)
    # Use a simpler approach that works on all systems
    RECENT_DB=$(ls -t "$TARGET_DIR"/golden_test_*.db 2>/dev/null | head -1)
    
    # Check if the most recent golden database is less than 1 hour old
    if [ -n "$RECENT_DB" ] && [ -f "$RECENT_DB" ]; then
        # Get file age in seconds
        if command -v stat >/dev/null 2>&1; then
            # Linux/GNU stat
            FILE_TIME=$(stat -c %Y "$RECENT_DB" 2>/dev/null || stat -f %m "$RECENT_DB" 2>/dev/null)
        else
            FILE_TIME=0
        fi
        CURRENT_TIME=$(date +%s)
        AGE_SECONDS=$((CURRENT_TIME - FILE_TIME))
        
        # If file is less than 1 hour old (3600 seconds), skip creation
        if [ "$AGE_SECONDS" -lt 3600 ]; then
            echo "Recent golden database found: $RECENT_DB"
            echo "Database age: $((AGE_SECONDS / 60)) minutes"
            echo "Skipping creation. Delete existing golden databases if you want to recreate."
            echo "Golden database is ready for testing."
            exit 0
        fi
    fi
fi

# Generate timestamp-based version for new database
VERSION_TIMESTAMP=$(date +%Y%m%d_%H%M%S)
GOLDEN_DB_PATH="$TARGET_DIR/golden_test_${VERSION_TIMESTAMP}.db"

echo "Golden database version: $VERSION_TIMESTAMP"
echo "Golden database path: $GOLDEN_DB_PATH"

# Ensure target directory exists
mkdir -p "$TARGET_DIR"

# Set up the database URL for the golden database
export DATABASE_URL="sqlite://$GOLDEN_DB_PATH"

# Initialize the database with migrations
echo "Initializing database schema..."
cd neems-api
diesel --database-url="$DATABASE_URL" setup
cd ..

# Find neems-admin binary
if [ -n "$NEEMS_ADMIN_BIN" ]; then
    if [ -x "$NEEMS_ADMIN_BIN" ]; then
        echo "Using pre-set NEEMS_ADMIN_BIN: $NEEMS_ADMIN_BIN" >&2
    else
        echo "Error: Pre-set NEEMS_ADMIN_BIN '$NEEMS_ADMIN_BIN' is not executable or does not exist" >&2
        exit 1
    fi
else
    NEEMS_ADMIN_BIN=$(command -v neems-admin 2>/dev/null)

    if [ -z "$NEEMS_ADMIN_BIN" ]; then
        if [ -x "./bin/neems-admin" ]; then
            NEEMS_ADMIN_BIN="./bin/neems-admin"
        elif [ -x "./neems-admin" ]; then
            NEEMS_ADMIN_BIN="./neems-admin"
        else
            echo "Error: neems-admin not found in PATH, ./bin/neems-admin, or ./neems-admin" >&2
            exit 1
        fi
    fi
    echo "Found neems-admin at: $NEEMS_ADMIN_BIN" >&2
    export NEEMS_ADMIN_BIN
fi

echo "Creating admin setup..."

# Create Newtown Energy company (admin_init_fairing equivalent)
echo "Creating Newtown Energy company..."
if $NEEMS_ADMIN_BIN company add --name "Newtown Energy"; then
    echo "Created Newtown Energy company"
else
    echo "Failed to create Newtown Energy company, checking if it already exists..."
fi

# Get Newtown Energy company ID
NEWTOWN_ENERGY_ID=$($NEEMS_ADMIN_BIN company ls | grep "Newtown Energy" | sed 's/.*ID: \([0-9]*\).*/\1/' | head -1)

if [ -z "$NEWTOWN_ENERGY_ID" ]; then
    echo "Error: Could not find Newtown Energy company ID"
    echo "Available companies:"
    $NEEMS_ADMIN_BIN company ls
    exit 1
fi

echo "Newtown Energy ID: $NEWTOWN_ENERGY_ID"

# Create roles
echo "Creating roles..."
$NEEMS_ADMIN_BIN role add --name "newtown-admin" --description "Administrator for Newtown" || echo "Failed to create newtown-admin role"
$NEEMS_ADMIN_BIN role add --name "newtown-staff" --description "Staff member for Newtown" || echo "Failed to create newtown-staff role"  
$NEEMS_ADMIN_BIN role add --name "admin" --description "Administrator for Site Owner" || echo "Failed to create admin role"
$NEEMS_ADMIN_BIN role add --name "staff" --description "User" || echo "Failed to create staff role"

# Verify roles were created successfully
echo "Verifying roles exist..."
$NEEMS_ADMIN_BIN role ls

# Create admin user (matches create_admin_user_in_db)
echo "Creating admin user..."
$NEEMS_ADMIN_BIN user add --email "superadmin@example.com" --password "admin" --company-id "$NEWTOWN_ENERGY_ID" 2>/dev/null || echo "User superadmin@example.com already exists"

# Assign newtown-admin role to admin user
$NEEMS_ADMIN_BIN user add-role --email "superadmin@example.com" --role "newtown-admin" 2>/dev/null || echo "Admin role already assigned to superadmin@example.com"

echo "Creating test data..."

# Create test companies
echo "Creating test companies..."
$NEEMS_ADMIN_BIN company add --name "Test Company 1" 2>/dev/null || echo "Company 'Test Company 1' already exists"
$NEEMS_ADMIN_BIN company add --name "Test Company 2" 2>/dev/null || echo "Company 'Test Company 2' already exists"
$NEEMS_ADMIN_BIN company add --name "Removable LLC" 2>/dev/null || echo "Company 'Removable LLC' already exists"

# Get test company IDs
TEST_COMPANY1_ID=$($NEEMS_ADMIN_BIN company ls | grep "Test Company 1" | sed 's/.*ID: \([0-9]*\).*/\1/' | head -1)
TEST_COMPANY2_ID=$($NEEMS_ADMIN_BIN company ls | grep "Test Company 2" | sed 's/.*ID: \([0-9]*\).*/\1/' | head -1)

if [ -z "$TEST_COMPANY1_ID" ] || [ -z "$TEST_COMPANY2_ID" ]; then
    echo "Error: Could not find test company IDs"
    exit 1
fi

echo "Test Company IDs: Company1=$TEST_COMPANY1_ID, Company2=$TEST_COMPANY2_ID"

# Create standard test users (matches create_test_data function)
echo "Creating standard test users..."

# Standard test users with default "admin" password
create_test_user() {
    local email=$1
    local company_id=$2
    local role_name=$3
    
    echo "Creating user: $email"
    $NEEMS_ADMIN_BIN user add --email "$email" --password "admin" --company-id "$company_id" 2>/dev/null || echo "User $email already exists"
    
    # Assign role by name (not ID)
    $NEEMS_ADMIN_BIN user add-role --email "$email" --role "$role_name" 2>/dev/null || echo "Role $role_name already assigned to $email"
}

# Standard test users
create_test_user "user@testcompany.com" "$TEST_COMPANY1_ID" "admin"
create_test_user "user@company1.com" "$TEST_COMPANY1_ID" "admin"
create_test_user "user@company2.com" "$TEST_COMPANY2_ID" "admin"
create_test_user "user@empty.com" "$TEST_COMPANY1_ID" "admin"
create_test_user "admin@company1.com" "$TEST_COMPANY1_ID" "admin"
create_test_user "admin@company2.com" "$TEST_COMPANY2_ID" "admin"
create_test_user "staff@testcompany.com" "$TEST_COMPANY1_ID" "staff"
create_test_user "newtownadmin@newtown.com" "$NEWTOWN_ENERGY_ID" "newtown-admin"
create_test_user "newtownstaff@newtown.com" "$NEWTOWN_ENERGY_ID" "newtown-staff"

# Additional test users for login.rs tests
create_test_user "testuser@example.com" "$TEST_COMPANY1_ID" "staff"

# Create test users with custom passwords (matches create_test_user_with_password calls)
echo "Creating test users with custom passwords..."

create_custom_password_user() {
    local email=$1
    local company_id=$2
    local role_name=$3
    local password=$4
    
    echo "Creating user with custom password: $email"
    $NEEMS_ADMIN_BIN user add --email "$email" --password "$password" --company-id "$company_id" 2>/dev/null || echo "User $email already exists"
    
    # Assign role by name (not ID)
    $NEEMS_ADMIN_BIN user add-role --email "$email" --role "$role_name" 2>/dev/null || echo "Role $role_name already assigned to $email"
}

# Additional test users for secure_test.rs tests with custom passwords
create_custom_password_user "test_superadmin@example.com" "$NEWTOWN_ENERGY_ID" "admin" "adminpass"
create_custom_password_user "staff@example.com" "$TEST_COMPANY1_ID" "staff" "staffpass"
create_custom_password_user "newtown_superadmin@example.com" "$NEWTOWN_ENERGY_ID" "newtown-admin" "newtownpass"
create_custom_password_user "newtown_staff@example.com" "$NEWTOWN_ENERGY_ID" "newtown-staff" "newtownstaffpass"
create_custom_password_user "regular@example.com" "$TEST_COMPANY1_ID" "staff" "regularpass"

# Create user with multiple roles (admin_staff@example.com with both admin and staff roles)
echo "Creating multi-role user..."
$NEEMS_ADMIN_BIN user add --email "admin_staff@example.com" --password "adminstaff" --company-id "$TEST_COMPANY1_ID" 2>/dev/null || echo "User admin_staff@example.com already exists"
$NEEMS_ADMIN_BIN user add-role --email "admin_staff@example.com" --role "admin" 2>/dev/null || echo "Admin role already assigned to admin_staff@example.com"
$NEEMS_ADMIN_BIN user add-role --email "admin_staff@example.com" --role "staff" 2>/dev/null || echo "Staff role already assigned to admin_staff@example.com"

echo "Golden database v$VERSION_TIMESTAMP created successfully at: $GOLDEN_DB_PATH"
echo "You can now run tests with: cargo test --features test-staging"
echo ""
echo "Golden database contains:"
echo "  Companies: Newtown Energy, Test Company 1, Test Company 2, Removable LLC"
echo "  Roles: newtown-admin, newtown-staff, admin, staff"
echo "  Admin user: superadmin@example.com (password: admin)"
echo "  Test users: Various users for different test scenarios"
