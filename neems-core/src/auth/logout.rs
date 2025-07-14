/*

logout endpoint

*/

use rocket::{post, http::{Cookie, CookieJar, Status}, Route};
use diesel::prelude::*;
use crate::DbConn;
use crate::schema::sessions::dsl::*;

#[post("/1/logout")]
pub async fn logout(
    db: DbConn,
    cookies: &CookieJar<'_>,
) -> Status {
    // Get the cookie value first without holding a reference
    let cookie_value = cookies.get("session").map(|c| c.value().to_string());
    
    if let Some(session_id) = cookie_value {
        // Mark session as revoked in DB
        let _ = db.run(move |conn| {
            diesel::update(sessions.filter(id.eq(&session_id)))
                .set(revoked.eq(true))
                .execute(conn)
        }).await;
        
        // Remove cookie
        cookies.remove(Cookie::from("session"));
    }
    Status::Ok
}

pub fn routes() -> Vec<Route> {
    routes![logout]
}

