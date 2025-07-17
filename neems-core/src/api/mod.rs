pub mod general;
pub mod institution;
pub mod role;
pub mod user;

use rocket::Route;

pub fn routes() -> Vec<Route> {
    let mut routes = Vec::new();
    routes.extend(general::routes());
    routes.extend(institution::routes());
    routes.extend(role::routes());
    routes.extend(user::routes());
    routes
}