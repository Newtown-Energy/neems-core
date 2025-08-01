use diesel::prelude::*;
use rocket::http::{ContentType};
use rocket::local::asynchronous::Client;
use rocket::serde::json::json;
use rocket::tokio;

use neems_core::orm::testing::test_rocket;
use neems_core::models::{Company, Role, User, UserWithRoles};
use neems_core::schema::users::dsl::*;
use neems_core::schema::roles;
use neems_core::schema::user_roles;

/// Helper to login and get session cookie
async fn login_and_get_session(client: &Client) -> rocket::http::Cookie<'static> {
    let login_body = json!({
        "email": "superadmin@example.com",
        "password": "admin"
    });
    
    let response = client.post("/api/1/login")
        .header(ContentType::JSON)
        .body(login_body.to_string())
        .dispatch()
        .await;
    
    assert_eq!(response.status(), rocket::http::Status::Ok);
    response.cookies().get("session")
        .expect("Session cookie should be set")
        .clone()
        .into_owned()
}

/// Helper to create authenticated user and institution
async fn setup_authenticated_user(client: &Client) -> (i32, rocket::http::Cookie<'static>) {
    // Create institution with authentication
    let login_cookie = login_and_get_session(client).await;
    
    let new_comp = json!({ "name": "A Bogus Company" });
    let response = client.post("/api/1/companies")
        .header(ContentType::JSON)
        .cookie(login_cookie.clone())
        .body(new_comp.to_string())
        .dispatch()
        .await;
    
    assert!(response.status().code < 400, "Company creation failed");
    let company: Company = response.into_json().await.expect("valid JSON");
    let comp_id = company.id;
    
    (comp_id, login_cookie)
}



#[tokio::test]
async fn test_admin_user_is_created() {

    // Start Rocket with the admin fairing attached
    let rocket = test_rocket();
    let client = Client::tracked(rocket).await.expect("valid rocket instance");

    // Get a DB connection from the pool
    let conn = neems_core::orm::DbConn::get_one(client.rocket()).await
        .expect("get db connection");

    // Use the default admin email (from env or fallback)
    let admin_email = std::env::var("NEEMS_DEFAULT_USER").unwrap_or_else(|_| "superadmin@example.com".to_string());

    // Query for the admin user and verify it has the newtown-admin role
    let (found_user, has_admin_role) = conn.run(move |c| {
        // Find the admin user
        let user = users.filter(email.eq(admin_email))
            .first::<User>(c)
            .optional()
            .expect("user query should not fail");

        let has_role = if let Some(ref u) = user {
            // Check if the user has the newtown-admin role
            let role_exists = user_roles::table
                .inner_join(roles::table)
                .filter(user_roles::user_id.eq(u.id))
                .filter(roles::name.eq("newtown-admin"))
                .first::<(neems_core::models::UserRole, Role)>(c)
                .optional()
                .expect("role query should not fail");
            
            role_exists.is_some()
        } else {
            false
        };

        (user, has_role)
    }).await;

    assert!(found_user.is_some(), "Admin user should exist after fairing runs");
    assert!(has_admin_role, "Admin user should have the newtown-admin role");
}

#[rocket::async_test]
async fn test_create_user_requires_auth() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    
    // Test unauthenticated request fails
    let new_user = json!({
        "email": "testuser@example.com",
        "password_hash": "hashed_pw",
        "company_id": 1,
        "totp_secret": "SECRET123"
    });
    
    let response = client.post("/api/1/users")
        .header(ContentType::JSON)
        .body(new_user.to_string())
        .dispatch()
        .await;
    
    assert_eq!(response.status(), rocket::http::Status::Unauthorized);
    
    // Test authenticated request succeeds
    let session_cookie = login_and_get_session(&client).await;
    let (comp_id, _) = setup_authenticated_user(&client).await;
    
    let new_user_auth = json!({
        "email": "newuser@example.com",
        "password_hash": "hashed_pw",
        "company_id": comp_id,
        "totp_secret": "SECRET123",
        "role_names": ["staff"]
    });
    
    let response = client.post("/api/1/users")
        .header(ContentType::JSON)
        .cookie(session_cookie)
        .body(new_user_auth.to_string())
        .dispatch()
        .await;
    
    assert_eq!(response.status(), rocket::http::Status::Created);
}

#[rocket::async_test]
async fn test_list_users_requires_auth() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    
    // Test unauthenticated request fails
    let response = client.get("/api/1/users").dispatch().await;
    assert_eq!(response.status(), rocket::http::Status::Unauthorized);
    
    // Test authenticated request succeeds
    let session_cookie = login_and_get_session(&client).await;
    
    let response = client.get("/api/1/users")
        .cookie(session_cookie)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), rocket::http::Status::Ok);
    
    let list: Vec<UserWithRoles> = response.into_json().await.expect("valid JSON response");
    assert!(!list.is_empty()); // Should have at least the admin user
}

#[rocket::async_test]
async fn test_user_crud_endpoints() {
    let client = Client::tracked(test_rocket()).await.expect("valid rocket instance");
    let session_cookie = login_and_get_session(&client).await;
    let (comp_id, _) = setup_authenticated_user(&client).await;
    
    // Create a test user
    let new_user = json!({
        "email": "crudtest@example.com",
        "password_hash": "testhash",
        "company_id": comp_id,
        "totp_secret": "testsecret",
        "role_names": ["staff"]
    });
    
    let response = client.post("/api/1/users")
        .header(ContentType::JSON)
        .cookie(session_cookie.clone())
        .body(new_user.to_string())
        .dispatch()
        .await;
    
    assert_eq!(response.status(), rocket::http::Status::Created);
    let created_user: UserWithRoles = response.into_json().await.expect("valid user JSON");
    
    // Test GET single user
    let url = format!("/api/1/users/{}", created_user.id);
    let response = client.get(&url)
        .cookie(session_cookie.clone())
        .dispatch()
        .await;
    
    assert_eq!(response.status(), rocket::http::Status::Ok);
    let retrieved_user: UserWithRoles = response.into_json().await.expect("valid user JSON");
    assert_eq!(retrieved_user.id, created_user.id);
    assert_eq!(retrieved_user.email, "crudtest@example.com");
    
    // Test PUT update user
    let update_data = json!({
        "email": "updated@example.com",
        "totp_secret": "updatedsecret"
    });
    
    let response = client.put(&url)
        .header(ContentType::JSON)
        .cookie(session_cookie.clone())
        .body(update_data.to_string())
        .dispatch()
        .await;
    
    assert_eq!(response.status(), rocket::http::Status::Ok);
    let updated_user: UserWithRoles = response.into_json().await.expect("valid user JSON");
    assert_eq!(updated_user.email, "updated@example.com");
    assert_eq!(updated_user.totp_secret, Some("updatedsecret".to_string()));
    assert_eq!(updated_user.password_hash, "testhash"); // Should remain unchanged
    
    // Test DELETE user (should work as we're logged in as newtown-admin)
    let response = client.delete(&url)
        .cookie(session_cookie)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), rocket::http::Status::NoContent); // Should work as we're logged in as newtown-admin
}
