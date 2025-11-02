//! Tests for OData navigation routes.
//!
//! This module tests OData navigation properties that allow traversing
//! relationships between entities through dedicated endpoints.

use neems_api::{
    models::{Company, Role, Site, UserWithRoles},
    orm::testing::fast_test_rocket,
};
use rocket::{
    http::{ContentType, Status},
    local::asynchronous::Client,
};
use serde_json::json;

/// Helper to login as default admin and get session cookie
async fn login_admin(client: &Client) -> rocket::http::Cookie<'static> {
    let login_body = json!({
        "email": "superadmin@example.com",
        "password": "admin"
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

/// Helper to login with specific credentials and get session cookie
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

#[rocket::async_test]
async fn test_user_company_navigation() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get a user to test navigation
    let users_response = client.get("/api/1/Users").cookie(admin_cookie.clone()).dispatch().await;
    assert_eq!(users_response.status(), Status::Ok);

    let odata_response: serde_json::Value =
        users_response.into_json().await.expect("valid OData JSON");
    let users: Vec<UserWithRoles> =
        serde_json::from_value(odata_response["value"].clone()).expect("valid users array");

    if let Some(test_user) = users.first() {
        // Test navigation to user's company
        let nav_url = format!("/api/1/Users/{}/Company", test_user.id);
        let nav_response = client.get(&nav_url).cookie(admin_cookie.clone()).dispatch().await;

        assert_eq!(nav_response.status(), Status::Ok);
        let company: Company = nav_response.into_json().await.expect("valid company JSON");

        // Verify it's the correct company
        assert_eq!(company.id, test_user.company_id);
    }
}

#[rocket::async_test]
async fn test_user_roles_navigation() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get a user to test navigation
    let users_response = client.get("/api/1/Users").cookie(admin_cookie.clone()).dispatch().await;
    assert_eq!(users_response.status(), Status::Ok);

    let odata_response: serde_json::Value =
        users_response.into_json().await.expect("valid OData JSON");
    let users: Vec<UserWithRoles> =
        serde_json::from_value(odata_response["value"].clone()).expect("valid users array");

    if let Some(test_user) = users.first() {
        // Test navigation to user's roles
        let nav_url = format!("/api/1/Users/{}/Roles", test_user.id);
        let nav_response = client.get(&nav_url).cookie(admin_cookie.clone()).dispatch().await;

        assert_eq!(nav_response.status(), Status::Ok);
        let roles: Vec<Role> = nav_response.into_json().await.expect("valid roles JSON");

        // Verify roles match what's in the user object
        assert_eq!(roles.len(), test_user.roles.len());
        for role in roles {
            assert!(test_user.roles.iter().any(|r| r.id == role.id));
        }
    }
}

#[rocket::async_test]
async fn test_company_users_navigation() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get a company to test navigation
    let companies_response =
        client.get("/api/1/Companies").cookie(admin_cookie.clone()).dispatch().await;
    assert_eq!(companies_response.status(), Status::Ok);

    let odata_response: serde_json::Value =
        companies_response.into_json().await.expect("valid OData JSON");
    let companies: Vec<Company> =
        serde_json::from_value(odata_response["value"].clone()).expect("valid companies array");

    if let Some(test_company) = companies.first() {
        // Test navigation to company's users
        let nav_url = format!("/api/1/Companies/{}/Users", test_company.id);
        let nav_response = client.get(&nav_url).cookie(admin_cookie.clone()).dispatch().await;

        assert_eq!(nav_response.status(), Status::Ok);
        let users: Vec<UserWithRoles> = nav_response.into_json().await.expect("valid users JSON");

        // Verify all users belong to this company
        for user in users {
            assert_eq!(user.company_id, test_company.id);
        }
    }
}

#[rocket::async_test]
async fn test_company_sites_navigation() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Get a company to test navigation
    let companies_response =
        client.get("/api/1/Companies").cookie(admin_cookie.clone()).dispatch().await;
    assert_eq!(companies_response.status(), Status::Ok);

    let odata_response: serde_json::Value =
        companies_response.into_json().await.expect("valid OData JSON");
    let companies: Vec<Company> =
        serde_json::from_value(odata_response["value"].clone()).expect("valid companies array");

    if let Some(test_company) = companies.first() {
        // Test navigation to company's sites
        let nav_url = format!("/api/1/Companies/{}/Sites", test_company.id);
        let nav_response = client.get(&nav_url).cookie(admin_cookie.clone()).dispatch().await;

        assert_eq!(nav_response.status(), Status::Ok);
        let sites: Vec<Site> = nav_response.into_json().await.expect("valid sites JSON");

        // Verify all sites belong to this company
        for site in sites {
            assert_eq!(site.company_id, test_company.id);
        }
    }
}

#[rocket::async_test]
async fn test_navigation_authorization() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");

    // Login as a regular user from a specific company
    let user_cookie = login_user(&client, "admin@company1.com", "admin").await;

    // Get another user from a different company
    let admin_cookie = login_admin(&client).await;
    let users_response = client.get("/api/1/Users").cookie(admin_cookie.clone()).dispatch().await;

    let odata_response: serde_json::Value =
        users_response.into_json().await.expect("valid OData JSON");
    let users: Vec<UserWithRoles> =
        serde_json::from_value(odata_response["value"].clone()).expect("valid users array");

    // Find a user from a different company (get superadmin user)
    if let Some(other_user) = users.iter().find(|u| u.email == "superadmin@example.com") {
        // Try to access other user's company (should fail for company admin)
        let nav_url = format!("/api/1/Users/{}/Company", other_user.id);
        let nav_response = client.get(&nav_url).cookie(user_cookie.clone()).dispatch().await;

        assert_eq!(nav_response.status(), Status::Forbidden);
    }
}

#[rocket::async_test]
async fn test_navigation_not_found() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");
    let admin_cookie = login_admin(&client).await;

    // Test with non-existent user ID
    let nav_response = client
        .get("/api/1/Users/99999/Company")
        .cookie(admin_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(nav_response.status(), Status::NotFound);

    // Test with non-existent company ID
    let nav_response =
        client.get("/api/1/Companies/99999/Users").cookie(admin_cookie).dispatch().await;

    // Company endpoints return 200 with empty array for non-existent company
    // because authorization passes (superadmin can access any company)
    // but the query returns no results
    assert_eq!(nav_response.status(), Status::Ok);
}

#[rocket::async_test]
async fn test_navigation_requires_authentication() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");

    // Test navigation endpoints without authentication
    let endpoints = vec![
        "/api/1/Users/1/Company",
        "/api/1/Users/1/Roles",
        "/api/1/Companies/1/Users",
        "/api/1/Companies/1/Sites",
    ];

    for endpoint in endpoints {
        let response = client.get(endpoint).dispatch().await;
        assert_eq!(
            response.status(),
            Status::Unauthorized,
            "Endpoint {} should require auth",
            endpoint
        );
    }
}

#[rocket::async_test]
async fn test_newtown_staff_access_to_navigation() {
    let client = Client::tracked(fast_test_rocket()).await.expect("valid rocket instance");

    // Login as newtown-staff user
    let staff_cookie = login_user(&client, "newtownstaff@newtown.com", "admin").await;

    // Get any user
    let admin_cookie = login_admin(&client).await;
    let users_response = client.get("/api/1/Users").cookie(admin_cookie.clone()).dispatch().await;

    let odata_response: serde_json::Value =
        users_response.into_json().await.expect("valid OData JSON");
    let users: Vec<UserWithRoles> =
        serde_json::from_value(odata_response["value"].clone()).expect("valid users array");

    if let Some(test_user) = users.first() {
        // newtown-staff should be able to access any user's company
        let nav_url = format!("/api/1/Users/{}/Company", test_user.id);
        let nav_response = client.get(&nav_url).cookie(staff_cookie.clone()).dispatch().await;

        assert_eq!(nav_response.status(), Status::Ok);

        // newtown-staff should be able to access any user's roles
        let nav_url = format!("/api/1/Users/{}/Roles", test_user.id);
        let nav_response = client.get(&nav_url).cookie(staff_cookie).dispatch().await;

        assert_eq!(nav_response.status(), Status::Ok);
    }
}
