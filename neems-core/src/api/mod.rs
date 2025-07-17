pub mod fixphrase;
pub mod institution;
pub mod role;
pub mod status;
pub mod user;

use rocket::Route;

pub fn routes() -> Vec<Route> {
    let mut routes = Vec::new();
    routes.extend(fixphrase::routes());
    routes.extend(institution::routes());
    routes.extend(role::routes());
    routes.extend(status::routes());
    routes.extend(user::routes());
    routes
}