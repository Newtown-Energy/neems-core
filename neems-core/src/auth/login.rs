/*

login endpoint

 */

use chrono::Utc;
use diesel::prelude::*;
use rocket::{post, Route, http::{Cookie, CookieJar, SameSite, Status}, serde::json::Json};
use rocket::serde::Deserialize;
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

#[derive(Clone, Deserialize)]
pub struct LoginRequest {
    username: String,
    password_hash: String, // format is "argon2:<salt>:<hash>"
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

async fn find_user_by_username<D: DbRunner>(db: &D, username: &str) -> Result<Option<User>, Status> {
    let username = username.to_owned(); // Make an owned String
    db.run(move |conn| {
        users::table
            .filter(users::username.eq(username))
            .first::<User>(conn)
            .optional()
    })
    .await
    .map_err(|_| Status::InternalServerError)
}

/// Verifies the provided password hash against the stored user's password hash.
///
/// The `provided_hash` string must be of the form `<function>:<salt>:<hash>`.
/// For example, for Argon2, it would be `"argon2:<salt>:<hash>"`.
///
/// This function simply compares the provided hash string to the user's stored hash string.
/// In a real-world scenario, you should verify the password using a password hashing library
/// and compare the hash of the provided password (after extracting salt and function) to the stored hash.
///
/// # Arguments
/// * `provided_hash` - The hash string provided by the client, formatted as described.
/// * `user` - The user record from the database, whose `password_hash` field is also in the same format.
///
/// # Returns
/// * `true` if the hashes match, otherwise `false`.
fn verify_password(provided_hash: &str, user: &User) -> bool {
    provided_hash == user.password_hash
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
        .path("/");
    cookies.add(cookie);
}

pub async fn process_login<D: DbRunner>(
    db: &D,
    cookies: &CookieJar<'_>,
    login: &LoginRequest,
) -> Result<Status, Status> {
    let user = match find_user_by_username(db, &login.username).await? {
        Some(user) => user,
        None => return Err(Status::Unauthorized),
    };

    if !verify_password(&login.password_hash, &user) {
        return Err(Status::Unauthorized);
    }

    let session_token = create_and_store_session(db, user.id.unwrap()).await?;

    set_session_cookie(cookies, &session_token);

    Ok(Status::Ok)
}


#[post("/1/login", data = "<login>")]
pub async fn login(
    db: DbConn,
    cookies: &CookieJar<'_>,
    login: Json<LoginRequest>,
) -> Result<Status, Status> {
    process_login(&db, cookies, &login).await
}


#[get("/1/hello")]
pub async fn secure_hello(db: DbConn, cookies: &CookieJar<'_>) -> Result<String, Status> {
    if let Some(user) = AuthenticatedUser::from_cookies_and_db(cookies, &db).await {
        Ok(format!("Hello, {}!", user.username))
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
    use argon2::{Argon2, PasswordHasher};
    use argon2::password_hash::{SaltString};
    use rand_core::OsRng;

    use crate::db::{setup_test_db, setup_test_dbconn};
    use crate::institution::insert_institution;
    use crate::user::insert_user;
    use crate::models::{UserNoTime, User};
    use super::*;

    /// Hash a password and return a string
    ///
    /// Return string will be in the format "argon2:<salt>:<hash>".
    ///
    /// Note that this is a simplified version
    /// for testing purposes. In production, you would pass a salt in.
    fn hash_password(password: &str) -> String {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2.hash_password(password.as_bytes(), &salt)
            .expect("hashing should succeed")
            .hash
            .unwrap()
            .to_string();
        format!("argon2:{}:{}", salt.as_str(), password_hash)
    }


    /// Inserts a dummy institution and a dummy user, returning the inserted user.
    fn insert_dummy_user(conn: &mut diesel::SqliteConnection) -> User {
        let institution = insert_institution(conn, "Open Tech Strategies".to_string())
            .expect("insert dummy institution");

        let password_hash = hash_password("dummy password");

        let dummy_user = UserNoTime {
            username: "Karl Fogel".to_string(),
            email: "legofkarl@ots.com".to_string(),
            password_hash,
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
        let found = find_user_by_username(&fake_db, "Karl Fogel")
            .await
            .expect("db query should succeed");

        assert!(found.is_some());
        let found_user = found.unwrap();
        assert_eq!(found_user.username, inserted_user.username);
        assert_eq!(found_user.email, inserted_user.email);
        assert_eq!(found_user.password_hash, inserted_user.password_hash);
        assert_eq!(found_user.institution_id, inserted_user.institution_id);
    }
}

