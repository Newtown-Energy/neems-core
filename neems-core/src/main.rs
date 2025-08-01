// neems-core/src/main.rs

use std::env;
use rocket::info;
use rocket::error;

#[rocket::main]
async fn main() {
    match env::current_dir() {
        Ok(path) => info!("Current directory: {}", path.display()),
        Err(e) => error!("Error getting current directory: {}", e),
    };

    neems_core::rocket()
        .launch()
        .await
        .expect("Rocket server failed to launch");
}
