// neems-core/src/main.rs

use std::env;

#[rocket::main]
async fn main() {
    println!("{}",
	     match env::current_dir() {
		 Ok(path) => format!("Current directory: {}", path.display()),
		 Err(e) => format!("Error getting current directory: {}", e),
	     }
    );

    neems_core::rocket()
        .launch()
        .await
        .expect("Rocket server failed to launch");
}
