use diesel::prelude::*;
use rocket::http::ContentType;
use rocket::local::asynchronous::Client;
use rocket::serde::json::json;
use rocket::tokio;

use neems_api::models::{Role, User, UserWithRoles};
use neems_api::orm::testing::fast_test_rocket;
use neems_api::schema::roles;
use neems_api::schema::user_roles;
use neems_api::schema::users::dsl::*;

/// Helper to login and get session cookie
async fn login_and_get_session(client: &Client) -> rocket::http::Cookie<'static> {
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

    assert_eq!(response.status(), rocket::http::Status::Ok);
    response
        .cookies()
        .get("session")
        .expect("Session cookie should be set")
        .clone()
        .into_owned()
}

/// Helper to get a pre-existing test company ID from golden database
fn get_test_company_id() -> i32 {
    2 // Test Company 1 from golden database
}

#[tokio::test]
async fn test_admin_user_is_created() {
    // Start Rocket with the admin fairing attached
    let rocket = fast_test_rocket();
    let client = Client::tracked(rocket)
        .await
        .expect("valid rocket instance");

    // Get a DB connection from the pool
    let conn = neems_api::orm::DbConn::get_one(client.rocket())
        .await
        .expect("get db connection");

    // Use the default admin email (from env or fallback)
    let admin_email = std::env::var("NEEMS_DEFAULT_USER")
        .unwrap_or_else(|_| "superadmin@example.com".to_string());

    // Query for the admin user and verify it has the newtown-admin role
    let (found_user, has_admin_role) = conn
        .run(move |c| {
            // Find the admin user
            let user = users
                .filter(email.eq(admin_email))
                .first::<User>(c)
                .optional()
                .expect("user query should not fail");

            let has_role = if let Some(ref u) = user {
                // Check if the user has the newtown-admin role
                let role_exists = user_roles::table
                    .inner_join(roles::table)
                    .filter(user_roles::user_id.eq(u.id))
                    .filter(roles::name.eq("newtown-admin"))
                    .first::<(neems_api::models::UserRole, Role)>(c)
                    .optional()
                    .expect("role query should not fail");

                role_exists.is_some()
            } else {
                false
            };

            (user, has_role)
        })
        .await;

    assert!(
        found_user.is_some(),
        "Admin user should exist after fairing runs"
    );
    assert!(
        has_admin_role,
        "Admin user should have the newtown-admin role"
    );
}

#[rocket::async_test]
async fn test_create_user_requires_auth() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");

    // Test unauthenticated request fails
    let new_user = json!({
        "email": "newuser@test.com",
        "password_hash": "hashed_pw",
        "company_id": 1,
        "totp_secret": "SECRET123"
    });

    let response = client
        .post("/api/1/Users")
        .header(ContentType::JSON)
        .body(new_user.to_string())
        .dispatch()
        .await;

    assert_eq!(response.status(), rocket::http::Status::Unauthorized);

    // Test authenticated request succeeds
    let session_cookie = login_and_get_session(&client).await;
    
    let new_user_auth = json!({
        "email": "newuser@test.com",
        "password_hash": "hashed_pw",
        "company_id": get_test_company_id(),
        "totp_secret": "SECRET123",
        "role_names": ["staff"]
    });

    let response = client
        .post("/api/1/Users")
        .header(ContentType::JSON)
        .cookie(session_cookie)
        .body(new_user_auth.to_string())
        .dispatch()
        .await;

    assert_eq!(response.status(), rocket::http::Status::Created,
        "Authenticated user should be able to create new users");
}

#[rocket::async_test]
async fn test_list_users_requires_auth() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");

    // Test unauthenticated request fails
    let response = client.get("/api/1/Users").dispatch().await;
    assert_eq!(response.status(), rocket::http::Status::Unauthorized);

    // Test authenticated request succeeds
    let session_cookie = login_and_get_session(&client).await;

    let response = client
        .get("/api/1/Users")
        .cookie(session_cookie)
        .dispatch()
        .await;

    assert_eq!(response.status(), rocket::http::Status::Ok);

    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let list: Vec<UserWithRoles> = serde_json::from_value(odata_response["value"].clone()).expect("valid users array");
    assert!(!list.is_empty()); // Should have at least the admin user
}

#[rocket::async_test]
async fn test_user_crud_endpoints() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let session_cookie = login_and_get_session(&client).await;

    // Use existing golden DB user for all CRUD operations
    // We'll use user@empty.com which exists in Test Company 1
    
    // First get the user to find their ID
    let response = client
        .get("/api/1/Users")
        .cookie(session_cookie.clone())
        .dispatch()
        .await;
    
    assert_eq!(response.status(), rocket::http::Status::Ok);
    let odata_response: serde_json::Value = response.into_json().await.expect("valid OData JSON");
    let user_list: Vec<UserWithRoles> = serde_json::from_value(odata_response["value"].clone()).expect("valid users array");
    
    // Find the golden DB test user
    let test_user = user_list.iter()
        .find(|u| u.email == "user@empty.com")
        .expect("Golden DB user 'user@empty.com' should exist");

    // Test GET single user
    let url = format!("/api/1/Users/{}", test_user.id);
    let response = client
        .get(&url)
        .cookie(session_cookie.clone())
        .dispatch()
        .await;

    assert_eq!(response.status(), rocket::http::Status::Ok);
    let retrieved_user: UserWithRoles = response.into_json().await.expect("valid user JSON");
    assert_eq!(retrieved_user.id, test_user.id);
    assert_eq!(retrieved_user.email, "user@empty.com");

    // Test PUT update user (modifying golden DB is fine - next test gets fresh copy)
    let update_data = json!({
        "email": "user@modified.com",
        "totp_secret": "updatedsecret"
    });

    let response = client
        .put(&url)
        .header(ContentType::JSON)
        .cookie(session_cookie.clone())
        .body(update_data.to_string())
        .dispatch()
        .await;

    assert_eq!(response.status(), rocket::http::Status::Ok);
    let updated_user: UserWithRoles = response.into_json().await.expect("valid user JSON");
    assert_eq!(updated_user.email, "user@modified.com");
    assert_eq!(updated_user.totp_secret, Some("updatedsecret".to_string()));

    // Test DELETE user (should work as we're logged in as newtown-admin)
    let response = client.delete(&url).cookie(session_cookie.clone()).dispatch().await;

    assert_eq!(response.status(), rocket::http::Status::NoContent);
    
    // Verify deletion worked
    let response = client
        .get(&url)
        .cookie(session_cookie)
        .dispatch()
        .await;
    
    assert_eq!(response.status(), rocket::http::Status::NotFound);
}

#[rocket::async_test]
async fn test_create_user_with_nonexistent_email_should_succeed() {
    let client = Client::tracked(fast_test_rocket())
        .await
        .expect("valid rocket instance");
    let session_cookie = login_and_get_session(&client).await;

    // Use a simple non-existent email - golden DB is fresh for this test
    let unique_email = "brandnew@test.com";
    
    // First verify the email doesn't exist in the database
    let conn = neems_api::orm::DbConn::get_one(client.rocket())
        .await
        .expect("get db connection");
    
    let email_for_check = unique_email.to_string();
    let existing_user = conn.run(move |c| {
        neems_api::orm::user::get_user_by_email(c, &email_for_check)
    }).await.expect("database query should work");
    
    assert!(existing_user.is_none(), "Email should not exist in database");

    // Now try to create a user with this email - it should succeed
    let new_user = json!({
        "email": unique_email,
        "password_hash": "hashed_pw",
        "company_id": get_test_company_id(),
        "totp_secret": "testsecret",
        "role_names": ["staff"]
    });

    let response = client
        .post("/api/1/Users")
        .header(ContentType::JSON)
        .cookie(session_cookie)
        .body(new_user.to_string())
        .dispatch()
        .await;

    // This should succeed (Created), not fail with Conflict
    assert_eq!(response.status(), rocket::http::Status::Created, 
               "Creating user with unique email should succeed");
    
    let created_user: UserWithRoles = response.into_json().await.expect("valid user JSON");
    assert_eq!(created_user.email, unique_email);
}
