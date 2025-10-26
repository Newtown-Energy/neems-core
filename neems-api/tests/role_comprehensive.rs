//! Comprehensive role management tests combining CRUD operations and RBAC
//!
//! This module combines all role-related tests including:
//! - Basic CRUD operations (create, read, update, delete)
//! - Role-based access control (RBAC) for all operations
//! - Authentication requirements for all endpoints
//! - Error handling and edge cases

use neems_api::{models::Role, orm::testing::fast_test_rocket};
use rocket::{
    http::{ContentType, Status},
    local::asynchronous::Client,
};
use serde_json::json;

/// Unified helper to login with specific credentials and get session cookie
async fn login_user(client: &Client, email: &str, password: &str) -> rocket::http::Cookie<'static> {
    let login_body = json!({
        "email": email,
        "password": password
    });

    let response = client
        .post("/api/1/login")
        .header(ContentType::JSON)
        .body(login_body.to_string())
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    response
        .cookies()
        .get("session")
        .expect("Session cookie should be set")
        .clone()
        .into_owned()
}

/// Helper to create a test role
async fn create_test_role(
    client: &Client,
    admin_cookie: &rocket::http::Cookie<'static>,
    name: &str,
    description: Option<&str>,
) -> Role {
    let new_role = json!({
        "name": name,
        "description": description
    });

    let response = client
        .post("/api/1/Roles")
        .cookie(admin_cookie.clone())
        .json(&new_role)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    response.into_json().await.expect("valid role JSON")
}

// COMPREHENSIVE ROLE CRUD WITH AUTHENTICATION TESTS

#[rocket::async_test]
async fn test_role_crud_operations_and_authentication() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");

    // Test CREATE role requires authentication
    let new_role = json!({
        "name": "test_role",
        "description": "A test role"
    });

    let response = client
        .post("/api/1/Roles")
        .header(ContentType::JSON)
        .body(new_role.to_string())
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Unauthorized);

    // Test LIST roles requires authentication
    let response = client.get("/api/1/Roles").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    // Test GET role requires authentication
    let response = client.get("/api/1/Roles/1").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    // Test UPDATE role requires authentication
    let update_request = json!({"name": "Updated Name"});
    let response = client.put("/api/1/Roles/1").json(&update_request).dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    // Test DELETE role requires authentication
    let response = client.delete("/api/1/Roles/1").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);

    // Login as admin for authenticated tests
    let admin_cookie = login_user(&client, "superadmin@example.com", "admin").await;

    // Test authenticated CREATE role succeeds
    let response = client
        .post("/api/1/Roles")
        .header(ContentType::JSON)
        .cookie(admin_cookie.clone())
        .body(new_role.to_string())
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    let created_role: Role = response.into_json().await.expect("valid JSON response");
    assert_eq!(created_role.name, "test_role");
    assert_eq!(created_role.description, Some("A test role".to_string()));

    // Test authenticated LIST roles succeeds
    let response = client.get("/api/1/Roles").cookie(admin_cookie.clone()).dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let list: Vec<Role> = response.into_json().await.expect("valid JSON response");
    assert!(!list.is_empty()); // Should have at least the default roles + created role

    // Test GET single role
    let url = format!("/api/1/Roles/{}", created_role.id);
    let response = client.get(&url).cookie(admin_cookie.clone()).dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let retrieved_role: Role = response.into_json().await.expect("valid role JSON");
    assert_eq!(retrieved_role.id, created_role.id);
    assert_eq!(retrieved_role.name, "test_role");

    // Test UPDATE role
    let update_data = json!({
        "name": "updated_test_role",
        "description": "Updated description"
    });
    let response = client
        .put(&url)
        .header(ContentType::JSON)
        .cookie(admin_cookie.clone())
        .body(update_data.to_string())
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    let updated_role: Role = response.into_json().await.expect("valid role JSON");
    assert_eq!(updated_role.name, "updated_test_role");
    assert_eq!(updated_role.description, Some("Updated description".to_string()));

    // Test DELETE role
    let response = client.delete(&url).cookie(admin_cookie.clone()).dispatch().await;
    assert_eq!(response.status(), Status::NoContent);

    // Verify deletion worked
    let response = client.get(&url).cookie(admin_cookie).dispatch().await;
    assert_eq!(response.status(), Status::NotFound);
}

#[rocket::async_test]
async fn test_role_update_operations() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_user(&client, "superadmin@example.com", "admin").await;

    // Create a test role for update operations
    let created_role =
        create_test_role(&client, &admin_cookie, "Original Name", Some("Original description"))
            .await;

    let url = format!("/api/1/Roles/{}", created_role.id);

    // Test partial update - only name
    let update_request = json!({"name": "Updated Name"});
    let response = client
        .put(&url)
        .cookie(admin_cookie.clone())
        .json(&update_request)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    let updated_role: Role = response.into_json().await.expect("valid role JSON");
    assert_eq!(updated_role.name, "Updated Name");
    assert_eq!(updated_role.description, Some("Original description".to_string())); // Unchanged

    // Test partial update - only description
    let update_request = json!({"description": "New description"});
    let response = client
        .put(&url)
        .cookie(admin_cookie.clone())
        .json(&update_request)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    let updated_role: Role = response.into_json().await.expect("valid role JSON");
    assert_eq!(updated_role.name, "Updated Name"); // Unchanged from previous update
    assert_eq!(updated_role.description, Some("New description".to_string()));

    // Test setting description to null
    let update_request = json!({"description": serde_json::Value::Null});
    let response = client
        .put(&url)
        .cookie(admin_cookie.clone())
        .json(&update_request)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    let updated_role: Role = response.into_json().await.expect("valid role JSON");
    assert_eq!(updated_role.name, "Updated Name"); // Unchanged
    assert_eq!(updated_role.description, None); // Now null

    // Test empty update request (no fields to update)
    let update_request = json!({});
    let response = client.put(&url).cookie(admin_cookie).json(&update_request).dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let updated_role: Role = response.into_json().await.expect("valid role JSON");
    assert_eq!(updated_role.name, "Updated Name"); // Should remain unchanged
    assert_eq!(updated_role.description, None); // Should remain unchanged
}

#[rocket::async_test]
async fn test_role_error_conditions() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_user(&client, "superadmin@example.com", "admin").await;

    // Test GET non-existent role
    let response = client.get("/api/1/Roles/99999").cookie(admin_cookie.clone()).dispatch().await;
    assert_eq!(response.status(), Status::NotFound);

    // Test UPDATE non-existent role
    let update_request = json!({"name": "Updated Name"});
    let response = client
        .put("/api/1/Roles/99999")
        .cookie(admin_cookie.clone())
        .json(&update_request)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::NotFound);

    // Test DELETE non-existent role
    let response = client.delete("/api/1/Roles/99999").cookie(admin_cookie).dispatch().await;
    assert_eq!(response.status(), Status::NotFound);
}

#[rocket::async_test]
async fn test_role_full_crud_cycle() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_user(&client, "superadmin@example.com", "admin").await;

    // CREATE a role
    let created_role =
        create_test_role(&client, &admin_cookie, "Full Cycle Role", Some("Testing full API CRUD"))
            .await;
    assert_eq!(created_role.name, "Full Cycle Role");
    assert_eq!(created_role.description, Some("Testing full API CRUD".to_string()));

    // READ the role by ID
    let get_url = format!("/api/1/Roles/{}", created_role.id);
    let get_response = client.get(&get_url).cookie(admin_cookie.clone()).dispatch().await;
    assert_eq!(get_response.status(), Status::Ok);
    let read_role: Role = get_response.into_json().await.expect("valid role JSON");
    assert_eq!(read_role.id, created_role.id);
    assert_eq!(read_role.name, "Full Cycle Role");

    // UPDATE the role
    let update_request = json!({
        "name": "Updated Full Cycle Role",
        "description": "Updated description"
    });
    let put_response = client
        .put(&get_url)
        .cookie(admin_cookie.clone())
        .json(&update_request)
        .dispatch()
        .await;
    assert_eq!(put_response.status(), Status::Ok);
    let updated_role: Role = put_response.into_json().await.expect("valid role JSON");
    assert_eq!(updated_role.name, "Updated Full Cycle Role");
    assert_eq!(updated_role.description, Some("Updated description".to_string()));

    // Verify the update by reading again
    let get_response2 = client.get(&get_url).cookie(admin_cookie.clone()).dispatch().await;
    assert_eq!(get_response2.status(), Status::Ok);
    let verified_role: Role = get_response2.into_json().await.expect("valid role JSON");
    assert_eq!(verified_role.name, "Updated Full Cycle Role");

    // DELETE the role
    let delete_response = client.delete(&get_url).cookie(admin_cookie.clone()).dispatch().await;
    assert_eq!(delete_response.status(), Status::NoContent);

    // Verify deletion
    let get_response3 = client.get(&get_url).cookie(admin_cookie).dispatch().await;
    assert_eq!(get_response3.status(), Status::NotFound);
}

// RBAC TESTS FOR ROLE OPERATIONS

#[rocket::async_test]
async fn test_role_operations_require_newtown_admin() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_user(&client, "superadmin@example.com", "admin").await;

    // Create test roles first for update/delete tests
    let role_for_admin = create_test_role(
        &client,
        &admin_cookie,
        "Role for Admin Test",
        Some("Will not be accessible"),
    )
    .await;
    let role_for_staff = create_test_role(
        &client,
        &admin_cookie,
        "Role for Staff Test",
        Some("Will not be accessible"),
    )
    .await;
    let role_for_newtown_admin = create_test_role(
        &client,
        &admin_cookie,
        "Role for Newtown Admin",
        Some("Will be accessible"),
    )
    .await;

    // Test with regular admin from Test Company 1 (should fail all operations)
    let admin_session = login_user(&client, "admin@company1.com", "admin").await;

    // CREATE should fail
    let new_role = json!({
        "name": "Test Role",
        "description": "Should not be created"
    });
    let response = client
        .post("/api/1/Roles")
        .cookie(admin_session.clone())
        .json(&new_role)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Forbidden);

    // UPDATE should fail
    let update_request = json!({"name": "Updated Name"});
    let url = format!("/api/1/Roles/{}", role_for_admin.id);
    let response = client
        .put(&url)
        .cookie(admin_session.clone())
        .json(&update_request)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Forbidden);

    // DELETE should fail
    let response = client.delete(&url).cookie(admin_session.clone()).dispatch().await;
    assert_eq!(response.status(), Status::Forbidden);

    // LIST and GET should succeed (all authenticated users can read roles)
    let response = client.get("/api/1/Roles").cookie(admin_session.clone()).dispatch().await;
    assert_eq!(response.status(), Status::Ok);

    let url = format!("/api/1/Roles/{}", role_for_admin.id);
    let response = client.get(&url).cookie(admin_session).dispatch().await;
    assert_eq!(response.status(), Status::Ok);

    // Test with newtown-staff (should fail CUD operations)
    let staff_session = login_user(&client, "newtownstaff@newtown.com", "admin").await;

    // CREATE should fail
    let response = client
        .post("/api/1/Roles")
        .cookie(staff_session.clone())
        .json(&new_role)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Forbidden);

    // UPDATE should fail
    let url = format!("/api/1/Roles/{}", role_for_staff.id);
    let response = client
        .put(&url)
        .cookie(staff_session.clone())
        .json(&update_request)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Forbidden);

    // DELETE should fail
    let response = client.delete(&url).cookie(staff_session.clone()).dispatch().await;
    assert_eq!(response.status(), Status::Forbidden);

    // LIST and GET should succeed
    let response = client.get("/api/1/Roles").cookie(staff_session.clone()).dispatch().await;
    assert_eq!(response.status(), Status::Ok);

    let response = client.get(&url).cookie(staff_session).dispatch().await;
    assert_eq!(response.status(), Status::Ok);

    // Test with newtown-admin (should succeed all operations)
    // CREATE should succeed
    let response = client
        .post("/api/1/Roles")
        .cookie(admin_cookie.clone())
        .json(&new_role)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    let created_role: Role = response.into_json().await.expect("valid role JSON");
    assert_eq!(created_role.name, "Test Role");

    // UPDATE should succeed
    let url = format!("/api/1/Roles/{}", role_for_newtown_admin.id);
    let response = client
        .put(&url)
        .cookie(admin_cookie.clone())
        .json(&update_request)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    let updated_role: Role = response.into_json().await.expect("valid role JSON");
    assert_eq!(updated_role.name, "Updated Name");

    // DELETE should succeed
    let response = client.delete(&url).cookie(admin_cookie.clone()).dispatch().await;
    assert_eq!(response.status(), Status::NoContent);

    // Verify role was deleted
    let get_response = client.get(&url).cookie(admin_cookie).dispatch().await;
    assert_eq!(get_response.status(), Status::NotFound);
}

#[rocket::async_test]
async fn test_all_authenticated_users_can_read_roles() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_user(&client, "superadmin@example.com", "admin").await;

    // Use existing golden database roles instead of creating new ones
    let response = client.get("/api/1/Roles").cookie(admin_cookie).dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let all_roles: Vec<Role> = response.into_json().await.expect("valid roles JSON");

    // Find an existing role from golden database (we know "admin" role exists)
    let existing_role = all_roles
        .iter()
        .find(|r| r.name == "admin")
        .expect("admin role should exist in golden database");

    // Test with regular admin
    let admin_session = login_user(&client, "admin@company1.com", "admin").await;

    // Should be able to list roles
    let response = client.get("/api/1/Roles").cookie(admin_session.clone()).dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let roles: Vec<Role> = response.into_json().await.expect("valid roles JSON");
    assert!(roles.len() >= 4); // At least the 4 default roles from golden DB

    // Should be able to get individual role using existing role
    let url = format!("/api/1/Roles/{}", existing_role.id);
    let response = client.get(&url).cookie(admin_session).dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let role: Role = response.into_json().await.expect("valid role JSON");
    assert_eq!(role.name, "admin");

    // Test with newtown-staff
    let staff_session = login_user(&client, "newtownstaff@newtown.com", "admin").await;

    // Should be able to list roles
    let response = client.get("/api/1/Roles").cookie(staff_session.clone()).dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let roles: Vec<Role> = response.into_json().await.expect("valid roles JSON");
    assert!(roles.len() >= 4);

    // Should be able to get individual role
    let response = client.get(&url).cookie(staff_session).dispatch().await;
    assert_eq!(response.status(), Status::Ok);

    // Test with regular user (non-admin)
    let user_session = login_user(&client, "staff@testcompany.com", "admin").await;

    // Should be able to list roles
    let response = client.get("/api/1/Roles").cookie(user_session.clone()).dispatch().await;
    assert_eq!(response.status(), Status::Ok);
    let roles: Vec<Role> = response.into_json().await.expect("valid roles JSON");
    assert!(roles.len() >= 4);

    // Should be able to get individual role
    let response = client.get(&url).cookie(user_session).dispatch().await;
    assert_eq!(response.status(), Status::Ok);
}
