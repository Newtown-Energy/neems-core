//! Tests for secure endpoints with role-based authentication.
//!
//! This module tests the secure endpoints that demonstrate different types of
//! authentication and authorization requirements. These tests are only compiled
//! when the `test-staging` feature is enabled.

#[cfg(feature = "test-staging")]
use rocket::http::{Status, ContentType};
#[cfg(feature = "test-staging")]
use rocket::local::asynchronous::Client;
#[cfg(feature = "test-staging")]
use serde_json::json;

#[cfg(feature = "test-staging")]
use neems_core::orm::testing::test_rocket;
#[cfg(feature = "test-staging")]
use neems_core::orm::{DbConn};
#[cfg(feature = "test-staging")]
use neems_core::orm::company::{insert_company, get_company_by_name};
#[cfg(feature = "test-staging")]
use neems_core::orm::user::insert_user;
#[cfg(feature = "test-staging")]
use neems_core::orm::user_role::assign_user_role_by_name;
#[cfg(feature = "test-staging")]
use neems_core::orm::role::insert_role;
#[cfg(feature = "test-staging")]
use neems_core::orm::login::hash_password;
#[cfg(feature = "test-staging")]
use neems_core::models::{UserNoTime, NewRole, CompanyNoTime};
#[cfg(feature = "test-staging")]
use neems_core::company::random_energy_company_names;

#[cfg(feature = "test-staging")]
/// Helper function to create test users with specific roles.
async fn setup_test_users(client: &Client) {
    let db_conn = DbConn::get_one(client.rocket()).await
        .expect("database connection for setup_test_users");
    
    db_conn.run(|conn| {
        // Get Newtown Energy company (should already exist)
        let newtown_energy = get_company_by_name(conn, &CompanyNoTime {
            name: "Newtown Energy".to_string(),
        }).expect("Failed to query Newtown Energy")
          .expect("Newtown Energy should exist");
        
        // Create a regular test company
        let regular_inst = insert_company(conn, random_energy_company_names(1)[0].to_string())
            .expect("Failed to insert company");

        // Create additional roles that might not exist
        let roles_to_create = vec![
            ("admin", "Administrator role"),
            ("staff", "Staff role"),
            ("newtown-admin", "Newtown administrator role"),
            ("newtown-staff", "Newtown staff role"),
        ];

        for (role_name, role_desc) in roles_to_create {
            let _ = insert_role(conn, NewRole {
                name: role_name.to_string(),
                description: Some(role_desc.to_string()),
            });
        }

        // Create test users with different roles and correct companys
        // Users with newtown roles must be at Newtown Energy company
        let test_users = vec![
            ("test_superadmin@example.com", "adminpass", vec!["admin"], regular_inst.id),
            ("staff@example.com", "staffpass", vec!["staff"], regular_inst.id),
            ("admin_staff@example.com", "adminstaff", vec!["admin", "staff"], regular_inst.id),
            ("newtown_superadmin@example.com", "newtownpass", vec!["newtown-admin", "admin"], newtown_energy.id),
            ("newtown_staff@example.com", "newtownstaffpass", vec!["newtown-staff"], newtown_energy.id),
            ("regular@example.com", "regularpass", vec!["staff"], regular_inst.id),
        ];

        for (email, password, roles, company_id) in test_users {
            let user = insert_user(conn, UserNoTime {
                email: email.to_string(),
                password_hash: hash_password(password),
                company_id,
                totp_secret: Some("dummy_secret".to_string()),
            }).expect("Failed to insert user");

            for role_name in roles {
                assign_user_role_by_name(conn, user.id, role_name)
                    .expect("Failed to assign role to user");
            }
        }
    }).await;
}

#[cfg(feature = "test-staging")]
/// Helper function to login as a specific user and get session cookie.
async fn login_as_user(client: &Client, email: &str, password: &str) -> rocket::http::Cookie<'static> {
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

#[cfg(feature = "test-staging")]
#[rocket::async_test]
async fn test_admin_only_endpoint_with_admin_user() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    setup_test_users(&client).await;
    
    let session_cookie = login_as_user(&client, "test_superadmin@example.com", "adminpass").await;
    
    let response = client.get("/api/1/test/admin-only")
        .cookie(session_cookie)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    
    let json_response: serde_json::Value = response.into_json().await
        .expect("valid JSON response");
    
    assert_eq!(json_response["required_role"], "admin");
    assert!(json_response["message"].as_str().unwrap().contains("test_superadmin@example.com"));
}

#[cfg(feature = "test-staging")]
#[rocket::async_test]
async fn test_admin_only_endpoint_with_non_admin_user() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    setup_test_users(&client).await;
    
    let session_cookie = login_as_user(&client, "staff@example.com", "staffpass").await;
    
    let response = client.get("/api/1/test/admin-only")
        .cookie(session_cookie)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Forbidden);
}

#[cfg(feature = "test-staging")]
#[rocket::async_test]
async fn test_staff_only_endpoint_with_staff_user() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    setup_test_users(&client).await;
    
    let session_cookie = login_as_user(&client, "staff@example.com", "staffpass").await;
    
    let response = client.get("/api/1/test/staff-only")
        .cookie(session_cookie)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    
    let json_response: serde_json::Value = response.into_json().await
        .expect("valid JSON response");
    
    assert_eq!(json_response["required_role"], "staff");
    assert!(json_response["message"].as_str().unwrap().contains("staff@example.com"));
}


#[cfg(feature = "test-staging")]
#[rocket::async_test]
async fn test_admin_and_staff_endpoint_with_both_roles() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    setup_test_users(&client).await;
    
    let session_cookie = login_as_user(&client, "admin_staff@example.com", "adminstaff").await;
    
    let response = client.get("/api/1/test/admin-and-staff")
        .cookie(session_cookie)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    
    let json_response: serde_json::Value = response.into_json().await
        .expect("valid JSON response");
    
    assert_eq!(json_response["required_roles"], json!(["admin", "staff"]));
    assert!(json_response["message"].as_str().unwrap().contains("admin_staff@example.com"));
}

#[cfg(feature = "test-staging")]
#[rocket::async_test]
async fn test_admin_and_staff_endpoint_with_only_admin() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    setup_test_users(&client).await;
    
    let session_cookie = login_as_user(&client, "test_superadmin@example.com", "adminpass").await;
    
    let response = client.get("/api/1/test/admin-and-staff")
        .cookie(session_cookie)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Forbidden);
}

#[cfg(feature = "test-staging")]
#[rocket::async_test]
async fn test_admin_and_staff_endpoint_with_only_staff() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    setup_test_users(&client).await;
    
    let session_cookie = login_as_user(&client, "staff@example.com", "staffpass").await;
    
    let response = client.get("/api/1/test/admin-and-staff")
        .cookie(session_cookie)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Forbidden);
}

#[cfg(feature = "test-staging")]
#[rocket::async_test]
async fn test_no_admin_allowed_endpoint_with_admin() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    setup_test_users(&client).await;
    
    let session_cookie = login_as_user(&client, "test_superadmin@example.com", "adminpass").await;
    
    let response = client.get("/api/1/test/no-admin-allowed")
        .cookie(session_cookie)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Forbidden);
}

#[cfg(feature = "test-staging")]
#[rocket::async_test]
async fn test_no_admin_allowed_endpoint_with_non_admin() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    setup_test_users(&client).await;
    
    let session_cookie = login_as_user(&client, "staff@example.com", "staffpass").await;
    
    let response = client.get("/api/1/test/no-admin-allowed")
        .cookie(session_cookie)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    
    let json_response: serde_json::Value = response.into_json().await
        .expect("valid JSON response");
    
    assert_eq!(json_response["forbidden_roles"], json!(["admin"]));
    assert!(json_response["message"].as_str().unwrap().contains("staff@example.com"));
}

#[cfg(feature = "test-staging")]
#[rocket::async_test]
async fn test_any_admin_or_staff_endpoint_with_admin() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    setup_test_users(&client).await;
    
    let session_cookie = login_as_user(&client, "test_superadmin@example.com", "adminpass").await;
    
    let response = client.get("/api/1/test/any-admin-or-staff")
        .cookie(session_cookie)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    
    let json_response: serde_json::Value = response.into_json().await
        .expect("valid JSON response");
    
    assert_eq!(json_response["accepted_roles"], json!(["admin", "staff", "newtown-admin"]));
}

#[cfg(feature = "test-staging")]
#[rocket::async_test]
async fn test_any_admin_or_staff_endpoint_with_staff() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    setup_test_users(&client).await;
    
    let session_cookie = login_as_user(&client, "staff@example.com", "staffpass").await;
    
    let response = client.get("/api/1/test/any-admin-or-staff")
        .cookie(session_cookie)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
}

#[cfg(feature = "test-staging")]
#[rocket::async_test]
async fn test_any_admin_or_staff_endpoint_with_newtown_admin() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    setup_test_users(&client).await;
    
    let session_cookie = login_as_user(&client, "newtown_superadmin@example.com", "newtownpass").await;
    
    let response = client.get("/api/1/test/any-admin-or-staff")
        .cookie(session_cookie)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
}


#[cfg(feature = "test-staging")]
#[rocket::async_test]
async fn test_newtown_admin_only_endpoint() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    setup_test_users(&client).await;
    
    let session_cookie = login_as_user(&client, "newtown_superadmin@example.com", "newtownpass").await;
    
    let response = client.get("/api/1/test/newtown-admin-only")
        .cookie(session_cookie)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    
    let json_response: serde_json::Value = response.into_json().await
        .expect("valid JSON response");
    
    assert_eq!(json_response["required_role"], "newtown-admin");
    assert!(json_response["message"].as_str().unwrap().contains("newtown_superadmin@example.com"));
}

#[cfg(feature = "test-staging")]
#[rocket::async_test]
async fn test_newtown_staff_only_endpoint() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    setup_test_users(&client).await;
    
    let session_cookie = login_as_user(&client, "newtown_staff@example.com", "newtownstaffpass").await;
    
    let response = client.get("/api/1/test/newtown-staff-only")
        .cookie(session_cookie)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), Status::Ok);
    
    let json_response: serde_json::Value = response.into_json().await
        .expect("valid JSON response");
    
    assert_eq!(json_response["required_role"], "newtown-staff");
    assert!(json_response["message"].as_str().unwrap().contains("newtown_staff@example.com"));
}

#[cfg(feature = "test-staging")]
#[rocket::async_test]
async fn test_unauthenticated_access_to_all_endpoints() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    setup_test_users(&client).await;
    
    let test_endpoints = vec![
        "/api/1/test/admin-only",
        "/api/1/test/staff-only",
        "/api/1/test/newtown-admin-only",
        "/api/1/test/newtown-staff-only",
        "/api/1/test/admin-and-staff",
        "/api/1/test/no-admin-allowed",
        "/api/1/test/any-admin-or-staff",
    ];
    
    for endpoint in test_endpoints {
        let response = client.get(endpoint)
            .dispatch()
            .await;
        
        assert_eq!(response.status(), Status::Unauthorized, 
                   "Endpoint {} should require authentication", endpoint);
    }
}


// When test-staging feature is disabled, this module is empty
#[cfg(not(feature = "test-staging"))]
mod empty_module {
    // This module intentionally left empty when test-staging is disabled
}