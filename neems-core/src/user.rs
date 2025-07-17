

use rocket::tokio;

#[tokio::test]
async fn test_admin_user_is_created() {
    use crate::orm::test_rocket;
    use rocket::local::asynchronous::Client;

    // Start Rocket with the admin fairing attached
    let rocket = test_rocket();
    let client = Client::tracked(rocket).await.expect("valid rocket instance");

    // Get a DB connection from the pool
    let conn = crate::orm::DbConn::get_one(client.rocket()).await
        .expect("get db connection");

    // Use the default admin email (from env or fallback)
    let admin_email = std::env::var("NEEMS_DEFAULT_USER").unwrap_or_else(|_| "admin@example.com".to_string());

    // Query for the admin user and verify it has the newtown-admin role
    let (found_user, has_admin_role) = conn.run(move |c| {
        use diesel::prelude::*;
        use crate::models::{User, Role};
        use crate::schema::users::dsl::*;
        use crate::schema::roles;
        use crate::schema::user_roles;

        // Find the admin user
        let user = users.filter(email.eq(admin_email))
            .first::<User>(c)
            .optional()
            .expect("user query should not fail");

        let has_role = if let Some(ref u) = user {
            // Check if the user has the newtown-admin role
            let role_exists = user_roles::table
                .inner_join(roles::table)
                .filter(user_roles::user_id.eq(u.id.expect("user should have id")))
                .filter(roles::name.eq("newtown-admin"))
                .first::<(crate::models::UserRole, Role)>(c)
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
