//! Integration tests for user role management endpoints
//!
//! This module tests the user role management API endpoints:
//! - GET /api/1/users/{id}/roles - Retrieve user's roles
//! - POST /api/1/users/{id}/roles - Add role to user
//! - DELETE /api/1/users/{id}/roles - Remove role from user
//!
//! Tests cover all authorization rules:
//! 1. newtown-staff and newtown-admin roles are reserved for Newtown Energy company
//! 2. newtown-admin can set any user's role to anything  
//! 3. newtown-staff can set any user's role except newtown-admin
//! 4. admin can set another user's role to admin if target user is at same company
//! 5. Users must have at least one role

use neems_api::models::{Company, CompanyInput, Role, User, UserInput};
use neems_api::orm::DbConn;
use neems_api::orm::company::{get_company_by_name, insert_company};
use neems_api::orm::login::hash_password;
use neems_api::orm::testing::test_rocket;
use neems_api::orm::user::insert_user;
use neems_api::orm::user_role::{assign_user_role_by_name, get_user_roles};

use rocket::http::{ContentType, Cookie, Status};
use rocket::local::asynchronous::Client;
use serde_json::json;

/// Test data structure for holding created users with different roles
#[derive(Debug)]
#[allow(dead_code)]
struct TestUsers {
    pub newtown_admin: User,
    pub newtown_staff: User,
    pub regular_admin: User,
    pub regular_user: User,
    pub other_company_admin: User,
    pub other_company_user: User,
}

/// Test companys
#[derive(Debug)]
#[allow(dead_code)]
struct TestCompanies {
    pub newtown_energy: Company,
    pub regular_company: Company,
    pub other_company: Company,
}

/// Creates comprehensive test data with users of various roles across different companys
async fn setup_test_data(client: &Client) -> (TestUsers, TestCompanies) {
    let db_conn = DbConn::get_one(client.rocket())
        .await
        .expect("database connection for setup_test_data");

    let (users, companies) = db_conn
        .run(|conn| {
            // Create test companys
            let newtown_energy = get_company_by_name(
                conn,
                &CompanyInput {
                    name: "Newtown Energy".to_string(),
                },
            )
            .expect("Failed to query Newtown Energy")
            .expect("Newtown Energy should exist");

            let regular_company = insert_company(conn, "Regular Corp".to_string())
                .expect("Failed to insert regular company");

            let other_company = insert_company(conn, "Other Corp".to_string())
                .expect("Failed to insert other company");

            // Create users with different roles
            let newtown_admin = insert_user(
                conn,
                UserInput {
                    email: "newtown_admin@newtownenergy.com".to_string(),
                    password_hash: hash_password("password123"),
                    company_id: newtown_energy.id,
                    totp_secret: Some("secret1".to_string()),
                },
            )
            .expect("Failed to insert newtown admin");
            assign_user_role_by_name(conn, newtown_admin.id, "newtown-admin")
                .expect("Failed to assign newtown-admin role");

            let newtown_staff = insert_user(
                conn,
                UserInput {
                    email: "newtown_staff@newtownenergy.com".to_string(),
                    password_hash: hash_password("password123"),
                    company_id: newtown_energy.id,
                    totp_secret: Some("secret2".to_string()),
                },
            )
            .expect("Failed to insert newtown staff");
            assign_user_role_by_name(conn, newtown_staff.id, "newtown-staff")
                .expect("Failed to assign newtown-staff role");

            let regular_admin = insert_user(
                conn,
                UserInput {
                    email: "admin@regularcorp.com".to_string(),
                    password_hash: hash_password("password123"),
                    company_id: regular_company.id,
                    totp_secret: Some("secret3".to_string()),
                },
            )
            .expect("Failed to insert regular admin");
            assign_user_role_by_name(conn, regular_admin.id, "admin")
                .expect("Failed to assign admin role");

            let regular_user = insert_user(
                conn,
                UserInput {
                    email: "user@regularcorp.com".to_string(),
                    password_hash: hash_password("password123"),
                    company_id: regular_company.id,
                    totp_secret: Some("secret4".to_string()),
                },
            )
            .expect("Failed to insert regular user");
            assign_user_role_by_name(conn, regular_user.id, "staff")
                .expect("Failed to assign user role");

            let other_company_admin = insert_user(
                conn,
                UserInput {
                    email: "admin@othercorp.com".to_string(),
                    password_hash: hash_password("password123"),
                    company_id: other_company.id,
                    totp_secret: Some("secret5".to_string()),
                },
            )
            .expect("Failed to insert other company admin");
            assign_user_role_by_name(conn, other_company_admin.id, "admin")
                .expect("Failed to assign admin role to other company admin");

            let other_company_user = insert_user(
                conn,
                UserInput {
                    email: "user@othercorp.com".to_string(),
                    password_hash: hash_password("password123"),
                    company_id: other_company.id,
                    totp_secret: Some("secret6".to_string()),
                },
            )
            .expect("Failed to insert other company user");
            assign_user_role_by_name(conn, other_company_user.id, "staff")
                .expect("Failed to assign user role to other company user");

            let users = TestUsers {
                newtown_admin,
                newtown_staff,
                regular_admin,
                regular_user,
                other_company_admin,
                other_company_user,
            };

            let companies = TestCompanies {
                newtown_energy,
                regular_company,
                other_company,
            };

            (users, companies)
        })
        .await;

    (users, companies)
}

/// Helper function to login as a specific user and get session cookie
async fn login_as_user(client: &Client, email: &str, password: &str) -> Cookie<'static> {
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

#[tokio::test]
async fn test_get_user_roles_requires_authentication() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let (users, _) = setup_test_data(&client).await;

    // Test unauthenticated request
    let response = client
        .get(format!("/api/1/users/{}/roles", users.regular_user.id))
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Unauthorized);
}

#[tokio::test]
async fn test_get_user_roles_user_can_view_own_roles() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let (users, _) = setup_test_data(&client).await;

    // Login as regular user
    let session_cookie = login_as_user(&client, "user@regularcorp.com", "password123").await;

    // User can view their own roles
    let response = client
        .get(format!("/api/1/users/{}/roles", users.regular_user.id))
        .cookie(session_cookie)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let roles: Vec<Role> = response.into_json().await.expect("valid JSON response");
    assert_eq!(roles.len(), 1);
    assert_eq!(roles[0].name, "staff");
}

#[tokio::test]
async fn test_get_user_roles_user_cannot_view_others_roles() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let (users, _) = setup_test_data(&client).await;

    // Login as regular user
    let session_cookie = login_as_user(&client, "user@regularcorp.com", "password123").await;

    // User cannot view other user's roles
    let response = client
        .get(format!(
            "/api/1/users/{}/roles",
            users.other_company_user.id
        ))
        .cookie(session_cookie)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Forbidden);
}

#[tokio::test]
async fn test_get_user_roles_admin_can_view_any_roles() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let (users, _) = setup_test_data(&client).await;

    // Login as newtown admin
    let session_cookie =
        login_as_user(&client, "newtown_admin@newtownenergy.com", "password123").await;

    // Admin can view any user's roles
    let response = client
        .get(format!(
            "/api/1/users/{}/roles",
            users.other_company_user.id
        ))
        .cookie(session_cookie)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
    let roles: Vec<Role> = response.into_json().await.expect("valid JSON response");
    assert_eq!(roles.len(), 1);
    assert_eq!(roles[0].name, "staff");
}

#[tokio::test]
async fn test_add_user_role_requires_authentication() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let (users, _) = setup_test_data(&client).await;

    let request_body = json!({
        "role_name": "admin"
    });

    let url = format!("/api/1/users/{}/roles", users.regular_user.id);
    let response = client.post(&url).json(&request_body).dispatch().await;

    assert_eq!(response.status(), Status::Unauthorized);
}

#[tokio::test]
async fn test_newtown_admin_can_assign_any_role() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let (users, _) = setup_test_data(&client).await;

    // Login as newtown admin
    let session_cookie =
        login_as_user(&client, "newtown_admin@newtownenergy.com", "password123").await;

    // Newtown admin can assign any role to any user
    let request_body = json!({
        "role_name": "admin"
    });

    let url = format!("/api/1/users/{}/roles", users.regular_user.id);
    let response = client
        .post(&url)
        .cookie(session_cookie)
        .json(&request_body)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);

    // Verify the role was assigned
    let db_conn = DbConn::get_one(client.rocket())
        .await
        .expect("database connection");
    let roles = db_conn
        .run(move |conn| get_user_roles(conn, users.regular_user.id))
        .await
        .expect("Failed to get user roles");

    assert_eq!(roles.len(), 2); // user + admin
    let role_names: Vec<&str> = roles.iter().map(|r| r.name.as_str()).collect();
    assert!(role_names.contains(&"staff"));
    assert!(role_names.contains(&"admin"));
}

#[tokio::test]
async fn test_newtown_staff_can_assign_non_admin_roles() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let (users, _) = setup_test_data(&client).await;

    // Login as newtown staff
    let session_cookie =
        login_as_user(&client, "newtown_staff@newtownenergy.com", "password123").await;

    // Newtown staff can assign non-admin roles
    let request_body = json!({
        "role_name": "admin"
    });

    let url = format!("/api/1/users/{}/roles", users.regular_user.id);
    let response = client
        .post(&url)
        .cookie(session_cookie)
        .json(&request_body)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
}

#[tokio::test]
async fn test_newtown_staff_cannot_assign_newtown_admin_role() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let (users, _) = setup_test_data(&client).await;

    // Login as newtown staff
    let session_cookie =
        login_as_user(&client, "newtown_staff@newtownenergy.com", "password123").await;

    // Newtown staff cannot assign newtown-admin role
    let request_body = json!({
        "role_name": "newtown-admin"
    });

    let url = format!("/api/1/users/{}/roles", users.regular_user.id);
    let response = client
        .post(&url)
        .cookie(session_cookie)
        .json(&request_body)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Forbidden);
}

#[tokio::test]
async fn test_regular_admin_can_assign_admin_to_same_company() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let (users, _) = setup_test_data(&client).await;

    // Login as regular admin
    let session_cookie = login_as_user(&client, "admin@regularcorp.com", "password123").await;

    // Regular admin can assign admin role to user in same company
    let request_body = json!({
        "role_name": "admin"
    });

    let url = format!("/api/1/users/{}/roles", users.regular_user.id);
    let response = client
        .post(&url)
        .cookie(session_cookie)
        .json(&request_body)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);
}

#[tokio::test]
async fn test_regular_admin_cannot_assign_admin_to_different_company() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let (users, _) = setup_test_data(&client).await;

    // Login as regular admin
    let session_cookie = login_as_user(&client, "admin@regularcorp.com", "password123").await;

    // Regular admin cannot assign admin role to user in different company
    let request_body = json!({
        "role_name": "admin"
    });

    let url = format!("/api/1/users/{}/roles", users.other_company_user.id);
    let response = client
        .post(&url)
        .cookie(session_cookie)
        .json(&request_body)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Forbidden);
}

#[tokio::test]
async fn test_regular_admin_cannot_assign_newtown_roles() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let (users, _) = setup_test_data(&client).await;

    // Login as regular admin
    let session_cookie = login_as_user(&client, "admin@regularcorp.com", "password123").await;

    // Regular admin cannot assign newtown-specific roles
    let request_body = json!({
        "role_name": "newtown-staff"
    });

    let url = format!("/api/1/users/{}/roles", users.regular_user.id);
    let response = client
        .post(&url)
        .cookie(session_cookie)
        .json(&request_body)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Forbidden);
}

#[tokio::test]
async fn test_newtown_roles_reserved_for_newtown_energy_users() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let (users, _) = setup_test_data(&client).await;

    // Login as newtown admin
    let session_cookie =
        login_as_user(&client, "newtown_admin@newtownenergy.com", "password123").await;

    // Cannot assign newtown role to user from different company
    let request_body = json!({
        "role_name": "newtown-staff"
    });

    let url = format!("/api/1/users/{}/roles", users.regular_user.id);
    let response = client
        .post(&url)
        .cookie(session_cookie)
        .json(&request_body)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Forbidden);
}

#[tokio::test]
async fn test_regular_user_cannot_assign_roles() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let (users, _) = setup_test_data(&client).await;

    // Login as regular user
    let session_cookie = login_as_user(&client, "user@regularcorp.com", "password123").await;

    // Regular user cannot assign roles
    let request_body = json!({
        "role_name": "staff"
    });

    let url = format!("/api/1/users/{}/roles", users.other_company_user.id);
    let response = client
        .post(&url)
        .cookie(session_cookie)
        .json(&request_body)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Forbidden);
}

#[tokio::test]
async fn test_remove_user_role_requires_authentication() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let (users, _) = setup_test_data(&client).await;

    let request_body = json!({
        "role_name": "admin"
    });

    let url = format!("/api/1/users/{}/roles", users.regular_admin.id);
    let response = client.delete(&url).json(&request_body).dispatch().await;

    assert_eq!(response.status(), Status::Unauthorized);
}

#[tokio::test]
async fn test_cannot_remove_last_role() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let (users, _) = setup_test_data(&client).await;

    // Login as newtown admin
    let session_cookie =
        login_as_user(&client, "newtown_admin@newtownenergy.com", "password123").await;

    // Cannot remove the only role from a user
    let request_body = json!({
        "role_name": "staff"
    });

    let url = format!("/api/1/users/{}/roles", users.regular_user.id);
    let response = client
        .delete(&url)
        .cookie(session_cookie)
        .json(&request_body)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::BadRequest);
}

#[tokio::test]
async fn test_remove_role_with_proper_authorization() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let (users, _) = setup_test_data(&client).await;

    // First add an additional role to the user
    let db_conn = DbConn::get_one(client.rocket())
        .await
        .expect("database connection");
    db_conn
        .run(move |conn| assign_user_role_by_name(conn, users.regular_admin.id, "staff"))
        .await
        .expect("Failed to assign additional role");

    // Login as newtown admin
    let session_cookie =
        login_as_user(&client, "newtown_admin@newtownenergy.com", "password123").await;

    // Now remove one role (leaving the other)
    let request_body = json!({
        "role_name": "staff"
    });

    let url = format!("/api/1/users/{}/roles", users.regular_admin.id);
    let response = client
        .delete(&url)
        .cookie(session_cookie)
        .json(&request_body)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Ok);

    // Verify the role was removed
    let db_conn = DbConn::get_one(client.rocket())
        .await
        .expect("database connection");
    let roles = db_conn
        .run(move |conn| get_user_roles(conn, users.regular_admin.id))
        .await
        .expect("Failed to get user roles");

    assert_eq!(roles.len(), 1);
    assert_eq!(roles[0].name, "admin");
}

#[tokio::test]
async fn test_newtown_staff_cannot_remove_newtown_admin_role() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let (users, _) = setup_test_data(&client).await;

    // Add user role to newtown admin so they have multiple roles
    let db_conn = DbConn::get_one(client.rocket())
        .await
        .expect("database connection");
    db_conn
        .run(move |conn| assign_user_role_by_name(conn, users.newtown_admin.id, "staff"))
        .await
        .expect("Failed to assign additional role");

    // Login as newtown staff
    let session_cookie =
        login_as_user(&client, "newtown_staff@newtownenergy.com", "password123").await;

    // Newtown staff cannot remove newtown-admin role
    let request_body = json!({
        "role_name": "newtown-admin"
    });

    let url = format!("/api/1/users/{}/roles", users.newtown_admin.id);
    let response = client
        .delete(&url)
        .cookie(session_cookie)
        .json(&request_body)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Forbidden);
}

#[tokio::test]
async fn test_regular_admin_authorization_for_role_removal() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let (users, _) = setup_test_data(&client).await;

    // Add user role to regular admin so they have multiple roles
    let db_conn = DbConn::get_one(client.rocket())
        .await
        .expect("database connection");
    db_conn
        .run(move |conn| assign_user_role_by_name(conn, users.regular_admin.id, "staff"))
        .await
        .expect("Failed to assign additional role");

    // Login as other company admin
    let session_cookie = login_as_user(&client, "admin@othercorp.com", "password123").await;

    // Admin from different company cannot remove admin role
    let request_body = json!({
        "role_name": "admin"
    });

    let url = format!("/api/1/users/{}/roles", users.regular_admin.id);
    let response = client
        .delete(&url)
        .cookie(session_cookie)
        .json(&request_body)
        .dispatch()
        .await;

    assert_eq!(response.status(), Status::Forbidden);
}

#[tokio::test]
async fn test_database_constraint_prevents_newtown_roles_for_non_newtown_users() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let (users, _) = setup_test_data(&client).await;

    // Try to directly assign a newtown role to a non-Newtown Energy user at the database level
    // This should fail due to the database trigger
    let db_conn = DbConn::get_one(client.rocket())
        .await
        .expect("database connection");

    let result = db_conn
        .run(move |conn| assign_user_role_by_name(conn, users.regular_user.id, "newtown-staff"))
        .await;

    // The database constraint should prevent this assignment
    assert!(
        result.is_err(),
        "Database constraint should prevent assigning newtown roles to non-Newtown users"
    );

    // Verify the error message contains our constraint message
    let error_msg = format!("{:?}", result.unwrap_err());
    assert!(
        error_msg.contains("Newtown roles") || error_msg.contains("can only be assigned"),
        "Error should mention Newtown roles restriction: {}",
        error_msg
    );
}

#[tokio::test]
async fn test_database_constraint_allows_newtown_roles_for_newtown_users() {
    let client = Client::tracked(test_rocket())
        .await
        .expect("valid rocket instance");
    let (users, _) = setup_test_data(&client).await;

    // Try to assign a newtown role to a Newtown Energy user at the database level
    // This should succeed as it's allowed by our constraint
    let db_conn = DbConn::get_one(client.rocket())
        .await
        .expect("database connection");

    let result = db_conn
        .run(move |conn| assign_user_role_by_name(conn, users.newtown_admin.id, "newtown-staff"))
        .await;

    // This should succeed since newtown_admin is from Newtown Energy company
    assert!(
        result.is_ok(),
        "Database should allow assigning newtown roles to Newtown Energy users"
    );

    // Verify the role was actually assigned
    let roles = db_conn
        .run(move |conn| get_user_roles(conn, users.newtown_admin.id))
        .await
        .expect("Failed to get user roles");

    let role_names: Vec<&str> = roles.iter().map(|r| r.name.as_str()).collect();
    assert!(role_names.contains(&"newtown-admin"));
    assert!(role_names.contains(&"newtown-staff"));
}
