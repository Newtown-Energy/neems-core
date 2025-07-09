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

#[post("/1/login", data = "<login>")]
pub async fn login(
    db: DbConn,
    cookies: &CookieJar<'_>,
    login: Json<LoginRequest>,
) -> Result<Status, Status> {
    // 1. Lookup user by username
    // 2. Verify password
    // 3. Check lockouts/rate limits (omitted for brevity)
    // 4. Create session token
    // 5. Insert session into DB
    // 6. Set cookie
    let login_clone = login.clone(); // Clone to move into async block
    let user_result = db.run(move |conn| {
	users::table
	    .filter(users::username.eq(&login_clone.username))
	    .first::<User>(conn)
	    .optional()
    }).await;

    let user = match user_result {
	Ok(Some(u)) => u,
	Ok(None) => {
	    // User not found: handle as unauthorized
	    return Err(Status::Unauthorized);
	}
	Err(_) => {
	    // Database error: handle as internal error
	    return Err(Status::InternalServerError);
	}
    };

    let password_ok = login.password_hash != ""; // TODO: Replace with actual password verification logic

    if !password_ok {
        return Err(Status::Unauthorized);
    }

    let session_token = generate_session_token();
    let now = Utc::now().naive_utc();

    let new_session = NewSession {
        id: session_token.clone(),
        user_id: user.id.unwrap(),
        created_at: now,
        expires_at: None, // or Some(now + duration)
        revoked: false,
    };

    let insert_result = db.run(move |conn| {
	diesel::insert_into(sessions::table)
	    .values(&new_session)
	    .execute(conn)
    }).await;

    match insert_result {
	Ok(_) => {
	    // Set cookie
	    let cookie = Cookie::build(("session", session_token))
		.http_only(true)
		.secure(true)
		.same_site(SameSite::Lax)
		.path("/");
	    cookies.add(cookie);
	    Ok(Status::Ok)
	},
	Err(_) => Err(Status::InternalServerError)
    }
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
