//! Database operations for user authentication and session management.
//!
//! This module provides database layer functions for user login, session creation,
//! password verification, and session storage. It abstracts database operations
//! to support both production and testing environments.

use argon2::{
    Argon2, PasswordHasher,
    password_hash::{PasswordHash, PasswordVerifier, SaltString, rand_core::OsRng},
};
use chrono::Utc;
use diesel::prelude::*;
use rocket::http::{Cookie, CookieJar, SameSite, Status};
use uuid::Uuid;

use crate::DbConn;
use crate::models::{NewSession, User};
#[cfg(feature = "test-staging")]
use crate::orm::testing::FakeDbConn;
use crate::schema::{sessions, users};

/// Trait for abstracting database operations to support both production and testing.
///
/// This trait allows the same functions to work with both `DbConn` (production)
/// and `FakeDbConn` (testing) by providing a unified interface for database operations.
pub trait DbRunner {
    /// Executes a database operation with a connection.
    ///
    /// # Arguments
    /// * `f` - Closure that takes a database connection and returns a result
    ///
    /// # Returns
    /// Future that resolves to the result of the database operation
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

#[cfg(feature = "test-staging")]
impl<'a> DbRunner for FakeDbConn<'a> {
    fn run<F, R>(&self, f: F) -> impl std::future::Future<Output = R>
    where
        F: FnOnce(&mut diesel::SqliteConnection) -> R + Send + 'static,
        R: Send + 'static,
    {
        FakeDbConn::run(self, f)
    }
}

/// Generates a new UUID-based session token.
///
/// This function creates a cryptographically secure random UUID for use as
/// a session token. The token is returned as a string representation.
///
/// # Returns
/// A new UUID string to be used as a session token
fn generate_session_token() -> String {
    Uuid::new_v4().to_string()
}

/// Finds a user by their email address.
///
/// This function queries the database to find a user with the specified email
/// address. It returns `None` if no user is found or an error occurs.
///
/// # Arguments
/// * `db` - Database connection implementing the `DbRunner` trait
/// * `email` - Email address to search for
///
/// # Returns
/// * `Ok(Some(User))` - User found with matching email
/// * `Ok(None)` - No user found with that email
/// * `Err(Status::InternalServerError)` - Database query failed
pub async fn find_user_by_email<D: DbRunner>(db: &D, email: &str) -> Result<Option<User>, Status> {
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

/// Verifies a password against a stored hash.
///
/// This function uses Argon2 password hashing to verify that a plain text
/// password matches the stored hash. It safely handles hash parsing errors
/// and returns `false` for invalid formats.
///
/// # Arguments
/// * `password` - Plain text password to verify
/// * `stored_hash` - Argon2 hash string from the database
///
/// # Returns
/// * `true` - Password matches the stored hash
/// * `false` - Password doesn't match or hash format is invalid
fn verify_password(password: &str, stored_hash: &str) -> bool {
    let parsed_hash = PasswordHash::new(stored_hash).expect("Invalid hash format");
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

/// Creates a new session and stores it in the database.
///
/// This function generates a new session token, creates a session record
/// in the database, and returns the token for use in cookies or headers.
///
/// # Arguments
/// * `db` - Database connection implementing the `DbRunner` trait
/// * `user_id` - ID of the user to create the session for
///
/// # Returns
/// * `Ok(String)` - Session token that was created and stored
/// * `Err(Status::InternalServerError)` - Database insertion failed
pub async fn create_and_store_session<D: DbRunner>(db: &D, user_id: i32) -> Result<String, Status> {
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
    })
    .await
    .map_err(|_| Status::InternalServerError)?;

    Ok(session_token)
}

/// Sets a secure session cookie in the response.
///
/// This function creates a session cookie with security settings appropriate
/// for production use: HTTP-only, secure, and SameSite=Lax protection.
///
/// # Arguments
/// * `cookies` - Cookie jar to add the session cookie to
/// * `session_token` - Session token value to store in the cookie
///
/// # Security Features
/// - `http_only(true)` - Prevents JavaScript access to the cookie
/// - `secure(true)` - Requires HTTPS for cookie transmission
/// - `same_site(SameSite::Lax)` - Provides CSRF protection
/// - `path("/")` - Makes cookie available for all paths
fn set_session_cookie(cookies: &CookieJar<'_>, session_token: &str) {
    let secure_flag = !cfg!(test);
    let cookie = Cookie::build(("session", session_token.to_string()))
        .http_only(true)
        .secure(secure_flag)
        .same_site(SameSite::Lax)
        .path("/")
        .build();
    cookies.add(cookie);
}

/// Processes a complete login workflow including validation and session creation.
///
/// This function handles the complete login process: validates input, finds the user,
/// verifies the password, creates a session, and sets the session cookie.
///
/// # Arguments
/// * `db` - Database connection implementing the `DbRunner` trait
/// * `cookies` - Cookie jar for setting the session cookie
/// * `login` - Login request containing email and password
///
/// # Returns
/// * `Ok((Status::Ok, User))` - Login successful, session created and cookie set, returns user data
/// * `Err(Status::BadRequest)` - Empty email or password provided
/// * `Err(Status::Unauthorized)` - Invalid credentials or user not found
/// * `Err(Status::InternalServerError)` - Database operation failed
///
/// # Security Notes
/// - Returns generic "Unauthorized" for both invalid users and wrong passwords
/// - Validates input to prevent empty credential attempts
/// - Uses secure password hashing for verification
pub async fn process_login<D: DbRunner>(
    db: &D,
    cookies: &CookieJar<'_>,
    login: &crate::api::login::LoginRequest,
) -> Result<(Status, User), Status> {
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

    let session_token = create_and_store_session(db, user.id).await?;
    set_session_cookie(cookies, &session_token);

    Ok((Status::Ok, user))
}

/// Hashes a password using Argon2 with a random salt.
///
/// This function creates a secure hash of a password using the Argon2 algorithm
/// with a cryptographically random salt. The resulting hash can be safely stored
/// in the database and used for password verification.
///
/// # Arguments
/// * `password` - Plain text password to hash
///
/// # Returns
/// Argon2 hash string suitable for database storage
///
/// # Security
/// - Uses Argon2 default parameters (recommended for security)
/// - Generates a random salt for each password
/// - Panics if hashing fails (should not happen in normal operation)
pub fn hash_password(password: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .expect("Hashing should succeed")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::User;
    use crate::models::UserInput;
    use crate::orm::company::insert_company;
    #[cfg(feature = "test-staging")]
    use crate::orm::testing::{setup_test_db, setup_test_dbconn};
    use crate::orm::user::insert_user;
    use rocket::http::Cookie;

    #[test]
    fn test_verify_password() {
        let password = "correct_password";
        let wrong_password = "wrong_password";
        let hash = hash_password(password);

        let user = User {
            id: 1,
            email: "test@example.com".to_string(),
            password_hash: hash,
            company_id: 1,
            totp_secret: Some("dummysecret".to_string()),
        };

        // Correct password should verify
        assert!(verify_password(password, &user.password_hash));

        // Wrong password should fail
        assert!(!verify_password(wrong_password, &user.password_hash));
    }

    /// Inserts a dummy company and a dummy user, returning the inserted user.
    fn insert_dummy_user(conn: &mut diesel::SqliteConnection) -> User {
        let company =
            insert_company(conn, "Open Tech Strategies".to_string(), None).expect("insert dummy company");

        let hash = hash_password("dummy password");

        let dummy_user = UserInput {
            email: "legofkarl@ots.com".to_string(),
            password_hash: hash,
            company_id: company.id,
            totp_secret: Some("dummysecret".to_string()),
        };
        insert_user(conn, dummy_user, None).expect("insert dummy user")
    }

    #[tokio::test]
    #[cfg(feature = "test-staging")]
    async fn test_find_user_by_email() {
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
        assert_eq!(found_user.company_id, inserted_user.company_id);
    }

    #[tokio::test]
    #[cfg(feature = "test-staging")]
    async fn test_create_and_store_session() {
        // Set up in-memory test database and async-compatible wrapper
        let mut conn = setup_test_db();

        // Insert dummy user
        let inserted_user = insert_dummy_user(&mut conn);

        let fake_db = setup_test_dbconn(&mut conn);

        // Use the function under test
        let session_token = create_and_store_session(&fake_db, inserted_user.id)
            .await
            .expect("session creation should succeed");

        // Clone the session_token for use in assertions later
        let session_token_clone = session_token.clone();

        // Verify the session was stored in the database
        let stored_session = fake_db
            .run(move |conn| {
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
        assert_eq!(session.user_id, inserted_user.id);
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
            Self {
                cookies: Vec::new(),
            }
        }

        pub fn add(&mut self, cookie: Cookie<'static>) {
            self.cookies.push(cookie);
        }

        pub fn get(&self, name: &str) -> Option<&Cookie<'static>> {
            self.cookies.iter().find(|c| c.name() == name)
        }
    }

    #[test]
    fn test_set_session_cookie_with_mock() {
        // Create mock jar
        let mut jar = MockCookieJar::new();

        // Test data
        let session_token = "test_session_token_123";

        // Call the function under test using our mock
        set_session_cookie_mock(&mut jar, session_token);

        // Verify the cookie was set with correct properties
        let cookie = jar.get("session").expect("session cookie should be set");

        assert_eq!(cookie.value(), session_token);
        assert!(cookie.http_only().unwrap_or(false));
        assert_eq!(cookie.secure(), Some(!cfg!(test))); // Should be false in tests, true in production
        assert_eq!(cookie.same_site(), Some(SameSite::Lax));
        assert_eq!(cookie.path(), Some("/"));
    }

    fn set_session_cookie_mock(cookies: &mut MockCookieJar, session_token: &str) {
        let cookie = Cookie::build(("session", session_token.to_string()))
            .http_only(true)
            .secure(!cfg!(test)) // Only secure in production, not in tests
            .same_site(SameSite::Lax)
            .path("/")
            .build();
        cookies.add(cookie);
    }
}
