use rocket::http::{Status, ContentType};
use rocket::local::asynchronous::Client;
use serde_json::json;

use neems_core::orm::testing::test_rocket;
use neems_core::models::{Role, Company, UserWithRoles};

/// Helper to login as default admin and get session cookie
async fn login_admin(client: &Client) -> rocket::http::Cookie<'static> {
    let login_body = json!({
        "email": "superadmin@example.com",
        "password": "admin"
    });
    
    let response = client.post("/api/1/login")
        .header(ContentType::JSON)
        .body(login_body.to_string())
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    response.cookies().get("session")
        .expect("Session cookie should be set")
        .clone()
        .into_owned()
}

/// Helper to create a test role
async fn create_test_role(client: &Client, admin_cookie: &rocket::http::Cookie<'static>, name: &str, description: Option<&str>) -> Role {
    let new_role = json!({
        "name": name,
        "description": description
    });
    
    let response = client.post("/api/1/roles")
        .cookie(admin_cookie.clone())
        .json(&new_role)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    response.into_json().await.expect("valid role JSON")
}

/// Helper to create a company
async fn create_company(client: &Client, admin_cookie: &rocket::http::Cookie<'static>, name: &str) -> Company {
    let new_comp = json!({"name": name});
    
    let response = client.post("/api/1/companies")
        .cookie(admin_cookie.clone())
        .json(&new_comp)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Created);
    response.into_json().await.expect("valid company JSON")
}

/// Helper to create a user and assign role
async fn create_user_with_role(
    client: &Client,
    admin_cookie: &rocket::http::Cookie<'static>,
    email: &str,
    company_id: i32,
    role_name: &str,
) -> UserWithRoles {
    // Create user with properly hashed password
    let password_hash = neems_core::orm::login::hash_password("admin");
    let new_user = json!({
        "email": email,
        "password_hash": password_hash,
        "company_id": company_id,
        "totp_secret": "",
        "role_names": [role_name]
    });
    
    let response = client.post("/api/1/users")
        .cookie(admin_cookie.clone())
        .json(&new_user)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Created);
    let created_user: UserWithRoles = response.into_json().await.expect("valid user JSON");
    
    // Role is already assigned during user creation, no need for separate assignment
    
    created_user
}

/// Helper to login with specific credentials and get session cookie
async fn login_user(client: &Client, email: &str, password: &str) -> rocket::http::Cookie<'static> {
    let login_body = json!({
        "email": email,
        "password": password
    });
    
    let response = client.post("/api/1/login")
        .header(ContentType::JSON)
        .body(login_body.to_string())
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    response.cookies().get("session")
        .expect("Session cookie should be set")
        .clone()
        .into_owned()
}

#[rocket::async_test]
async fn test_get_role_endpoint_requires_authentication() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    
    // Test unauthenticated request fails
    let response = client.get("/api/1/roles/1").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn test_get_role_endpoint_success() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;
    
    // Create a test role
    let created_role = create_test_role(&client, &admin_cookie, "Test Get Role", Some("A role for get testing")).await;
    
    // Get the role by ID
    let url = format!("/api/1/roles/{}", created_role.id);
    let response = client.get(&url)
        .cookie(admin_cookie)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    
    let retrieved_role: Role = response.into_json().await.expect("valid role JSON");
    assert_eq!(retrieved_role.id, created_role.id);
    assert_eq!(retrieved_role.name, "Test Get Role");
    assert_eq!(retrieved_role.description, Some("A role for get testing".to_string()));
}

#[rocket::async_test]
async fn test_get_role_endpoint_not_found() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;
    
    // Try to get a role that doesn't exist
    let response = client.get("/api/1/roles/99999")
        .cookie(admin_cookie)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::NotFound);
}

#[rocket::async_test]
async fn test_update_role_endpoint_requires_authentication() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    
    let update_request = json!({
        "name": "Updated Name"
    });
    
    // Test unauthenticated request fails
    let response = client.put("/api/1/roles/1")
        .json(&update_request)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn test_update_role_endpoint_success() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;
    
    // Create a test role
    let created_role = create_test_role(&client, &admin_cookie, "Original Name", Some("Original description")).await;
    
    // Update only the name
    let update_request = json!({
        "name": "Updated Name"
    });
    
    let url = format!("/api/1/roles/{}", created_role.id);
    let response = client.put(&url)
        .cookie(admin_cookie.clone())
        .json(&update_request)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    
    let updated_role: Role = response.into_json().await.expect("valid role JSON");
    assert_eq!(updated_role.id, created_role.id);
    assert_eq!(updated_role.name, "Updated Name");
    assert_eq!(updated_role.description, Some("Original description".to_string())); // Should be unchanged
    
    // Update only the description
    let update_request2 = json!({
        "description": "New description"
    });
    
    let response = client.put(&url)
        .cookie(admin_cookie.clone())
        .json(&update_request2)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    
    let updated_role2: Role = response.into_json().await.expect("valid role JSON");
    assert_eq!(updated_role2.name, "Updated Name"); // Should be unchanged from previous update
    assert_eq!(updated_role2.description, Some("New description".to_string()));
    
    // Update both fields
    let update_request3 = json!({
        "name": "Final Name",
        "description": "Final description"
    });
    
    let response = client.put(&url)
        .cookie(admin_cookie)
        .json(&update_request3)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    
    let updated_role3: Role = response.into_json().await.expect("valid role JSON");
    assert_eq!(updated_role3.name, "Final Name");
    assert_eq!(updated_role3.description, Some("Final description".to_string()));
}

#[rocket::async_test]
async fn test_update_role_endpoint_set_description_to_null() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;
    
    // Create a test role with description
    let created_role = create_test_role(&client, &admin_cookie, "Test Role", Some("Has description")).await;
    
    // Update description to null - explicitly setting to JSON null
    let update_request = json!({
        "description": serde_json::Value::Null
    });
    
    let url = format!("/api/1/roles/{}", created_role.id);
    let response = client.put(&url)
        .cookie(admin_cookie)
        .json(&update_request)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    
    let updated_role: Role = response.into_json().await.expect("valid role JSON");
    assert_eq!(updated_role.name, "Test Role"); // Should be unchanged
    assert_eq!(updated_role.description, None); // Should be null
}

#[rocket::async_test]
async fn test_update_role_endpoint_not_found() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;
    
    let update_request = json!({
        "name": "Updated Name"
    });
    
    // Try to update a role that doesn't exist
    let response = client.put("/api/1/roles/99999")
        .cookie(admin_cookie)
        .json(&update_request)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::NotFound);
}

#[rocket::async_test]
async fn test_delete_role_endpoint_requires_authentication() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    
    // Test unauthenticated request fails
    let response = client.delete("/api/1/roles/1").dispatch().await;
    assert_eq!(response.status(), Status::Unauthorized);
}

#[rocket::async_test]
async fn test_delete_role_endpoint_success() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;
    
    // Create a test role
    let created_role = create_test_role(&client, &admin_cookie, "Role to Delete", Some("Will be deleted")).await;
    
    // Verify role exists by getting it
    let get_url = format!("/api/1/roles/{}", created_role.id);
    let get_response = client.get(&get_url)
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;
    assert_eq!(get_response.status(), Status::Ok);
    
    // Delete the role
    let delete_url = format!("/api/1/roles/{}", created_role.id);
    let response = client.delete(&delete_url)
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::NoContent);
    
    // Verify role no longer exists
    let get_response_after = client.get(&get_url)
        .cookie(admin_cookie)
        .dispatch()
        .await;
    assert_eq!(get_response_after.status(), Status::NotFound);
}

#[rocket::async_test]
async fn test_delete_role_endpoint_not_found() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;
    
    // Try to delete a role that doesn't exist
    let response = client.delete("/api/1/roles/99999")
        .cookie(admin_cookie)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::NotFound);
}

#[rocket::async_test]
async fn test_role_crud_full_cycle_api() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;
    
    // Create a role
    let created_role = create_test_role(&client, &admin_cookie, "Full Cycle Role", Some("Testing full API CRUD")).await;
    assert_eq!(created_role.name, "Full Cycle Role");
    assert_eq!(created_role.description, Some("Testing full API CRUD".to_string()));
    
    // Read the role by ID
    let get_url = format!("/api/1/roles/{}", created_role.id);
    let get_response = client.get(&get_url)
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;
    
    assert_eq!(get_response.status(), Status::Ok);
    let read_role: Role = get_response.into_json().await.expect("valid role JSON");
    assert_eq!(read_role.id, created_role.id);
    assert_eq!(read_role.name, "Full Cycle Role");
    
    // Update the role
    let update_request = json!({
        "name": "Updated Full Cycle Role",
        "description": "Updated description"
    });
    
    let put_response = client.put(&get_url)
        .cookie(admin_cookie.clone())
        .json(&update_request)
        .dispatch()
        .await;
    
    assert_eq!(put_response.status(), Status::Ok);
    let updated_role: Role = put_response.into_json().await.expect("valid role JSON");
    assert_eq!(updated_role.name, "Updated Full Cycle Role");
    assert_eq!(updated_role.description, Some("Updated description".to_string()));
    
    // Verify the update by reading again
    let get_response2 = client.get(&get_url)
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;
    
    assert_eq!(get_response2.status(), Status::Ok);
    let verified_role: Role = get_response2.into_json().await.expect("valid role JSON");
    assert_eq!(verified_role.name, "Updated Full Cycle Role");
    
    // Delete the role
    let delete_response = client.delete(&get_url)
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;
    
    assert_eq!(delete_response.status(), Status::NoContent);
    
    // Verify deletion
    let get_response3 = client.get(&get_url)
        .cookie(admin_cookie)
        .dispatch()
        .await;
    
    assert_eq!(get_response3.status(), Status::NotFound);
}

#[rocket::async_test]
async fn test_update_role_endpoint_empty_request() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;
    
    // Create a test role
    let created_role = create_test_role(&client, &admin_cookie, "Unchanged Role", Some("Original description")).await;
    
    // Send empty update request (no fields to update)
    let update_request = json!({});
    
    let url = format!("/api/1/roles/{}", created_role.id);
    let response = client.put(&url)
        .cookie(admin_cookie)
        .json(&update_request)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    
    let updated_role: Role = response.into_json().await.expect("valid role JSON");
    assert_eq!(updated_role.id, created_role.id);
    assert_eq!(updated_role.name, "Unchanged Role"); // Should remain unchanged
    assert_eq!(updated_role.description, Some("Original description".to_string())); // Should remain unchanged
}

// RBAC Tests

#[rocket::async_test]
async fn test_create_role_requires_newtown_admin() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;
    
    // Get Newtown Energy company
    let companies_response = client.get("/api/1/companies")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;
    assert_eq!(companies_response.status(), Status::Ok);
    let companies: Vec<Company> = companies_response.into_json().await.expect("valid companies JSON");
    let newtown_company = companies.iter()
        .find(|c| c.name == "Newtown Energy")
        .expect("Newtown Energy company should exist");
    
    // Create a regular company and users with different roles
    let test_company = create_company(&client, &admin_cookie, "Test Company").await;
    
    // Test with regular admin (should fail)
    let _regular_admin = create_user_with_role(&client, &admin_cookie, "admin@testcompany.com", test_company.id, "admin").await;
    let admin_session = login_user(&client, "admin@testcompany.com", "admin").await;
    
    let new_role = json!({
        "name": "Test Role",
        "description": "Should not be created"
    });
    
    let response = client.post("/api/1/roles")
        .cookie(admin_session)
        .json(&new_role)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Forbidden);
    
    // Test with newtown-staff (should fail)
    let _newtown_staff = create_user_with_role(&client, &admin_cookie, "staff@newtown.com", newtown_company.id, "newtown-staff").await;
    let staff_session = login_user(&client, "staff@newtown.com", "admin").await;
    
    let response = client.post("/api/1/roles")
        .cookie(staff_session)
        .json(&new_role)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Forbidden);
    
    // Test with newtown-admin (should succeed)
    let response = client.post("/api/1/roles")
        .cookie(admin_cookie)
        .json(&new_role)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    let created_role: Role = response.into_json().await.expect("valid role JSON");
    assert_eq!(created_role.name, "Test Role");
}

#[rocket::async_test]
async fn test_update_role_requires_newtown_admin() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;
    
    // Create a test role first
    let created_role = create_test_role(&client, &admin_cookie, "Role to Update", Some("Original description")).await;
    
    // Get Newtown Energy company
    let companies_response = client.get("/api/1/companies")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;
    assert_eq!(companies_response.status(), Status::Ok);
    let companies: Vec<Company> = companies_response.into_json().await.expect("valid companies JSON");
    let newtown_company = companies.iter()
        .find(|c| c.name == "Newtown Energy")
        .expect("Newtown Energy company should exist");
    
    // Create a regular company and users with different roles
    let test_company = create_company(&client, &admin_cookie, "Test Company").await;
    
    let update_request = json!({
        "name": "Updated Name"
    });
    
    let url = format!("/api/1/roles/{}", created_role.id);
    
    // Test with regular admin (should fail)
    let _regular_admin = create_user_with_role(&client, &admin_cookie, "admin@testcompany.com", test_company.id, "admin").await;
    let admin_session = login_user(&client, "admin@testcompany.com", "admin").await;
    
    let response = client.put(&url)
        .cookie(admin_session)
        .json(&update_request)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Forbidden);
    
    // Test with newtown-staff (should fail)
    let _newtown_staff = create_user_with_role(&client, &admin_cookie, "staff@newtown.com", newtown_company.id, "newtown-staff").await;
    let staff_session = login_user(&client, "staff@newtown.com", "admin").await;
    
    let response = client.put(&url)
        .cookie(staff_session)
        .json(&update_request)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Forbidden);
    
    // Test with newtown-admin (should succeed)
    let response = client.put(&url)
        .cookie(admin_cookie)
        .json(&update_request)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    let updated_role: Role = response.into_json().await.expect("valid role JSON");
    assert_eq!(updated_role.name, "Updated Name");
}

#[rocket::async_test]
async fn test_delete_role_requires_newtown_admin() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;
    
    // Create test roles first
    let role_for_admin = create_test_role(&client, &admin_cookie, "Role for Admin Test", Some("Will not be deleted")).await;
    let role_for_staff = create_test_role(&client, &admin_cookie, "Role for Staff Test", Some("Will not be deleted")).await;
    let role_for_newtown_admin = create_test_role(&client, &admin_cookie, "Role for Newtown Admin", Some("Will be deleted")).await;
    
    // Get Newtown Energy company
    let companies_response = client.get("/api/1/companies")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;
    assert_eq!(companies_response.status(), Status::Ok);
    let companies: Vec<Company> = companies_response.into_json().await.expect("valid companies JSON");
    let newtown_company = companies.iter()
        .find(|c| c.name == "Newtown Energy")
        .expect("Newtown Energy company should exist");
    
    // Create a regular company and users with different roles
    let test_company = create_company(&client, &admin_cookie, "Test Company").await;
    
    // Test with regular admin (should fail)
    let _regular_admin = create_user_with_role(&client, &admin_cookie, "admin@testcompany.com", test_company.id, "admin").await;
    let admin_session = login_user(&client, "admin@testcompany.com", "admin").await;
    
    let url = format!("/api/1/roles/{}", role_for_admin.id);
    let response = client.delete(&url)
        .cookie(admin_session)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Forbidden);
    
    // Test with newtown-staff (should fail)
    let _newtown_staff = create_user_with_role(&client, &admin_cookie, "staff@newtown.com", newtown_company.id, "newtown-staff").await;
    let staff_session = login_user(&client, "staff@newtown.com", "admin").await;
    
    let url = format!("/api/1/roles/{}", role_for_staff.id);
    let response = client.delete(&url)
        .cookie(staff_session)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Forbidden);
    
    // Test with newtown-admin (should succeed)
    let url = format!("/api/1/roles/{}", role_for_newtown_admin.id);
    let response = client.delete(&url)
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::NoContent);
    
    // Verify role was deleted
    let get_response = client.get(&url)
        .cookie(admin_cookie)
        .dispatch()
        .await;
    assert_eq!(get_response.status(), Status::NotFound);
}

#[rocket::async_test]
async fn test_list_and_get_roles_allow_all_authenticated_users() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;
    
    // Create a test role first
    let created_role = create_test_role(&client, &admin_cookie, "Public Role", Some("Everyone can see this")).await;
    
    // Get Newtown Energy company
    let companies_response = client.get("/api/1/companies")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;
    assert_eq!(companies_response.status(), Status::Ok);
    let companies: Vec<Company> = companies_response.into_json().await.expect("valid companies JSON");
    let newtown_company = companies.iter()
        .find(|c| c.name == "Newtown Energy")
        .expect("Newtown Energy company should exist");
    
    // Create a regular company and users with different roles
    let test_company = create_company(&client, &admin_cookie, "Test Company").await;
    
    // Test with regular admin
    let _regular_admin = create_user_with_role(&client, &admin_cookie, "admin@testcompany.com", test_company.id, "admin").await;
    let admin_session = login_user(&client, "admin@testcompany.com", "admin").await;
    
    // Should be able to list roles
    let response = client.get("/api/1/roles")
        .cookie(admin_session.clone())
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    let roles: Vec<Role> = response.into_json().await.expect("valid roles JSON");
    assert!(roles.len() >= 4); // At least the 4 default roles + created test role
    
    // Should be able to get individual role
    let url = format!("/api/1/roles/{}", created_role.id);
    let response = client.get(&url)
        .cookie(admin_session)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    let role: Role = response.into_json().await.expect("valid role JSON");
    assert_eq!(role.name, "Public Role");
    
    // Test with newtown-staff
    let _newtown_staff = create_user_with_role(&client, &admin_cookie, "staff@newtown.com", newtown_company.id, "newtown-staff").await;
    let staff_session = login_user(&client, "staff@newtown.com", "admin").await;
    
    // Should be able to list roles
    let response = client.get("/api/1/roles")
        .cookie(staff_session.clone())
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    let roles: Vec<Role> = response.into_json().await.expect("valid roles JSON");
    assert!(roles.len() >= 4);
    
    // Should be able to get individual role
    let response = client.get(&url)
        .cookie(staff_session)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    
    // Test with regular user (non-admin)
    let _regular_user = create_user_with_role(&client, &admin_cookie, "user@testcompany.com", test_company.id, "user").await;
    let user_session = login_user(&client, "user@testcompany.com", "admin").await;
    
    // Should be able to list roles
    let response = client.get("/api/1/roles")
        .cookie(user_session.clone())
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
    let roles: Vec<Role> = response.into_json().await.expect("valid roles JSON");
    assert!(roles.len() >= 4);
    
    // Should be able to get individual role
    let response = client.get(&url)
        .cookie(user_session)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Ok);
}

#[rocket::async_test]
async fn test_unauthenticated_users_cannot_access_roles() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    
    // Test unauthenticated requests to all endpoints
    let test_role_data = json!({
        "name": "Test Role",
        "description": "Should not be created"
    });
    
    let update_data = json!({
        "name": "Updated Role"
    });
    
    // Create role - should fail
    let response = client.post("/api/1/roles")
        .json(&test_role_data)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Unauthorized);
    
    // List roles - should fail
    let response = client.get("/api/1/roles")
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Unauthorized);
    
    // Get role - should fail
    let response = client.get("/api/1/roles/1")
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Unauthorized);
    
    // Update role - should fail
    let response = client.put("/api/1/roles/1")
        .json(&update_data)
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Unauthorized);
    
    // Delete role - should fail
    let response = client.delete("/api/1/roles/1")
        .dispatch()
        .await;
    assert_eq!(response.status(), Status::Unauthorized);
}