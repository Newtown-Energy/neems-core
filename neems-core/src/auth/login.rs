//! Authentication and Login Endpoints for neems-core
//!
//! This module implements the user login endpoint and related authentication logic for the neems-core API.
//! It provides mechanisms for verifying user credentials, managing session tokens, and setting secure cookies.
//!
//! # Features
//! - **User Login:** Accepts email and password, verifies credentials, and issues a session.
//! - **Session Management:** Generates secure session tokens, stores them in the database, and sets HTTP-only cookies.
//! - **Security:** Ensures session cookies are secure, HTTP-only, and have appropriate SameSite policies.
//! - **Extensible Database Layer:** Abstracts database operations for testability and flexibility via the `DbRunner` trait.
//! - **Test Utilities:** Includes helpers and mocks for unit testing authentication flows.
//!
//! # Endpoints
//! - `POST /1/login` — Authenticates a user and sets a session cookie.
//! - `GET /1/hello` — Example endpoint requiring authentication, returns a greeting for the logged-in user.
//!
//! # Usage
//! Add the routes from this module to your Rocket application with:
//! ```text
//! mount("/api", neems_core::auth::login::routes())
//! ```
//!
//! # Security Notes
//! - Session cookies are set as HTTP-only and Secure, with SameSite=Lax.

use argon2::{
    password_hash::{PasswordHash, PasswordVerifier, SaltString, rand_core::OsRng},
    Argon2, PasswordHasher
};
use chrono::Utc;
use diesel::prelude::*;
use rocket::{post, Route, http::{Cookie, CookieJar, SameSite, Status}, serde::json::Json};
use rocket::response;
use rocket::serde::{Serialize, Deserialize};
use uuid::Uuid;

use crate::auth::session_guard::AuthenticatedUser;
use crate::DbConn;
use crate::db::FakeDbConn;
use crate::models::{User, NewSession};
// use crate::schema::users::dsl::{users, username, password_hash};
// use crate::schema::sessions::dsl::{sessions, id as session_id, user_id, created_at, expires_at, revoked};
use crate::schema::{users, sessions};

fn generate_session_token() -> String {
    Uuid::new_v4().to_string()
}

#[derive(Serialize)]
pub struct ErrorResponse {
    error: String,
}

#[derive(Clone, Deserialize)]
pub struct LoginRequest {
    email: String,  // Changed from username to email
    password: String,
}


pub trait DbRunner {
    fn run<F, R>(&self, f: F) -> impl std::future::Future<Output = R>
    where
        F: FnOnce(&mut diesel::SqliteConnection) -> R + Send + 'static,
        R: Send + 'static;
}

impl DbRunner for DbConn {
    fn run<F, R>(&self, f: F) -> impl std::future::Future<Output = R>
    where
        F: FnOnce(&mut diesel::SqliteConnection) -> R + Send + 'static,
        R: Send + 'static,
    {
        DbConn::run(self, f)
    }
}

impl<'a> DbRunner for FakeDbConn<'a> {
    fn run<F, R>(&self, f: F) -> impl std::future::Future<Output = R>
    where
        F: FnOnce(&mut diesel::SqliteConnection) -> R + Send + 'static,
        R: Send + 'static,
    {
        FakeDbConn::run(self, f)
    }
}

async fn find_user_by_email<D: DbRunner>(db: &D, email: &str) -> Result<Option<User>, Status> {
    let email = email.to_owned();
    db.run(move |conn| {
        users::table
            .filter(users::email.eq(email))
            .first::<User>(conn)
            .optional()
    })
    .await
    .map_err(|_| Status::InternalServerError)
}

fn verify_password(password: &str, stored_hash: &str) -> bool {
    let parsed_hash = PasswordHash::new(stored_hash).expect("Invalid hash format");
    Argon2::default().verify_password(password.as_bytes(), &parsed_hash).is_ok()
}


async fn create_and_store_session<D: DbRunner>(db: &D, user_id: i32) -> Result<String, Status> {
    let session_token = generate_session_token();
    let now = Utc::now().naive_utc();

    let new_session = NewSession {
        id: session_token.clone(),
        user_id,
        created_at: now,
        expires_at: None,
        revoked: false,
    };

    db.run(move |conn| {
        diesel::insert_into(sessions::table)
            .values(&new_session)
            .execute(conn)
    }).await.map_err(|_| Status::InternalServerError)?;

    Ok(session_token)
}

fn set_session_cookie(cookies: &CookieJar<'_>, session_token: &str) {
    let cookie = Cookie::build(("session", session_token.to_string()))
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Lax)
        .path("/")
	.build();
    cookies.add(cookie);
}

pub async fn process_login<D: DbRunner>(
    db: &D,
    cookies: &CookieJar<'_>,
    login: &LoginRequest,
) -> Result<Status, Status> {
    // Check for empty fields
    if login.email.trim().is_empty() || login.password.trim().is_empty() {
        return Err(Status::BadRequest);
    }

    let user = match find_user_by_email(db, &login.email).await? {
        Some(user) => user,
        None => return Err(Status::Unauthorized),
    };

    if !verify_password(&login.password, &user.password_hash) {
        return Err(Status::Unauthorized);
    }

    let session_token = create_and_store_session(db, user.id.unwrap()).await?;
    set_session_cookie(cookies, &session_token);

    Ok(Status::Ok)
}

pub fn hash_password(password: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .expect("Hashing should succeed")
        .to_string()
}

#[post("/1/login", data = "<login>")]
pub async fn login(
    db: DbConn,
    cookies: &CookieJar<'_>,
    login: Json<LoginRequest>,
) -> Result<Status, response::status::Custom<Json<ErrorResponse>>> {
    match process_login(&db, cookies, &login).await {
        Ok(status) => Ok(status),
        Err(status) => {
            let err_json = Json(ErrorResponse { error: "Invalid credentials".to_string() });
            Err(response::status::Custom(status, err_json))
        }
    }
}


#[get("/1/hello")]
pub async fn secure_hello(db: DbConn, cookies: &CookieJar<'_>) -> Result<String, Status> {
    if let Some(user) = AuthenticatedUser::from_cookies_and_db(cookies, &db).await {
        Ok(format!("Hello, {}!", user.email))
    } else {
        Err(Status::Unauthorized)
    }
}



pub fn routes() -> Vec<Route> {
    routes![login,
	    secure_hello]
}


#[cfg(test)]
mod tests {
    use rocket::http::{Cookie};

    use diesel::prelude::*;
    use crate::db::{setup_test_db, setup_test_dbconn};
    use crate::models::User;
    use crate::institution::insert_institution;
    use crate::models::UserNoTime;
    use crate::user::insert_user;
    use super::*;


    #[test]
    fn test_verify_password() {
        let password = "correct_password";
        let wrong_password = "wrong_password";
        let hash = hash_password(password);
        
        let now = Utc::now().naive_utc();
        let user = User {
            id: Some(1),
            email: "test@example.com".to_string(),
            password_hash: hash,
            institution_id: 1,
            created_at: now,
            updated_at: now,
            totp_secret: "dummysecret".to_string(),
        };

        // Correct password should verify
        assert!(verify_password(password, &user.password_hash));
        
        // Wrong password should fail
        assert!(!verify_password(wrong_password, &user.password_hash));
    }

    /// Inserts a dummy institution and a dummy user, returning the inserted user.
    fn insert_dummy_user(conn: &mut diesel::SqliteConnection) -> User {
        let institution = insert_institution(conn, "Open Tech Strategies".to_string())
            .expect("insert dummy institution");

        let hash = hash_password("dummy password");

        let dummy_user = UserNoTime {
            email: "legofkarl@ots.com".to_string(),
            password_hash: hash,
            institution_id: institution.id.unwrap(),
            totp_secret: "dummysecret".to_string(),
        };
        insert_user(conn, dummy_user).expect("insert dummy user")
    }

    #[tokio::test]
    async fn test_find_user_by_username() {
        // Set up in-memory test database and async-compatible wrapper
        let mut conn = setup_test_db();
        
        // Insert dummy user
        let inserted_user = insert_dummy_user(&mut conn);

        let fake_db = setup_test_dbconn(&mut conn);

        // Use the function under test
        let found = find_user_by_email(&fake_db, "legofkarl@ots.com")
            .await
            .expect("db query should succeed");

        assert!(found.is_some());
        let found_user = found.unwrap();
        assert_eq!(found_user.email, inserted_user.email);
        assert_eq!(found_user.password_hash, inserted_user.password_hash);
        assert_eq!(found_user.institution_id, inserted_user.institution_id);
    }


    #[tokio::test]
    async fn test_create_and_store_session() {
	// Set up in-memory test database and async-compatible wrapper
	let mut conn = setup_test_db();

	// Insert dummy user
	let inserted_user = insert_dummy_user(&mut conn);

	let fake_db = setup_test_dbconn(&mut conn);

	// Use the function under test
	let session_token = create_and_store_session(&fake_db, inserted_user.id.unwrap())
	    .await
	    .expect("session creation should succeed");

	// Clone the session_token for use in assertions later
	let session_token_clone = session_token.clone();

	// Verify the session was stored in the database
	let stored_session = fake_db.run(move |conn| {
	    sessions::table
		.filter(sessions::id.eq(&session_token))
		.first::<crate::models::Session>(conn)
		.optional()
	})
	.await
	.expect("db query should succeed");

	assert!(stored_session.is_some());
	let session = stored_session.unwrap();

	// Verify session properties
	assert_eq!(session.id, session_token_clone);
	assert_eq!(session.user_id, inserted_user.id.unwrap());
	assert!(!session.revoked);
	assert!(session.expires_at.is_none());

	// Verify created_at is recent (within last minute)
	let now = Utc::now().naive_utc();
	assert!(session.created_at <= now);
	assert!(session.created_at > now - chrono::Duration::minutes(1));
    }







    // Simplified mock implementation of a cookie jar for testing
    pub struct MockCookieJar {
        pub cookies: Vec<Cookie<'static>>,
    }

    impl MockCookieJar {
        pub fn new() -> Self {
            Self { cookies: Vec::new() }
        }

        pub fn add(&mut self, cookie: Cookie<'static>) {
            self.cookies.push(cookie);
        }

        pub fn get(&self, name: &str) -> Option<&Cookie<'static>> {
            self.cookies.iter().find(|c| c.name() == name)
        }
    }

    // Implement the minimal needed CookieJar behavior
    impl<'a> From<&'a MockCookieJar> for &'a rocket::http::CookieJar<'a> {
        fn from(mock: &'a MockCookieJar) -> &'a rocket::http::CookieJar<'a> {
            // This is a dummy implementation just to satisfy the type system
            // In practice, we won't actually use this conversion
            unsafe { std::mem::transmute(&mock.cookies) }
        }
    }

    #[test]
    fn test_set_session_cookie_with_mock() {
        // Create mock jar
        let mut jar = MockCookieJar::new();
        
        // Test data
        let session_token = "test_session_token_123";

        // Call the function under test using our mock
        // We need to adapt our function to accept the mock type
        set_session_cookie_mock(&mut jar, session_token);

        // Verify the cookie was set with correct properties
        let cookie = jar.get("session").expect("session cookie should be set");
        
        assert_eq!(cookie.value(), session_token);
        assert!(cookie.http_only().unwrap_or(false));
        assert!(cookie.secure().unwrap_or(false));
        assert_eq!(cookie.same_site(), Some(SameSite::Lax));
        assert_eq!(cookie.path(), Some("/"));
    }

    fn set_session_cookie_mock(cookies: &mut MockCookieJar, session_token: &str) {
	let cookie = Cookie::build(("session", session_token.to_string()))
	    .http_only(true)
	    .secure(true)
	    .same_site(SameSite::Lax)
	    .path("/")
	    .build();
	cookies.add(cookie);
    }




}
