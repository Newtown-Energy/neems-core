// neems-api/src/main.rs

use rocket::error;
use rocket::info;
use std::env;

#[rocket::main]
async fn main() {
    match env::current_dir() {
        Ok(path) => info!("Current directory: {}", path.display()),
        Err(e) => error!("Error getting current directory: {}", e),
    };

    neems_api::rocket()
        .launch()
        .await
        .expect("Rocket server failed to launch");
}
