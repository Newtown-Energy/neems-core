// neems-api/src/main.rs

use clap::Parser;
use rocket::error;
use rocket::info;
use std::env;

pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

#[derive(Parser)]
#[command(name = "neems-api")]
#[command(about = "NEEMS API server for energy monitoring systems")]
#[command(version)]
struct Cli {
    /// Show extended version information
    #[arg(long, action = clap::ArgAction::SetTrue)]
    version_info: bool,
}

#[rocket::main]
async fn main() {
    let cli = Cli::parse();

    // Handle --version-info flag
    if cli.version_info {
        println!("neems-api {}", built_info::PKG_VERSION);
        println!("Built: {}", built_info::BUILT_TIME_UTC);
        if let Some(commit) = built_info::GIT_COMMIT_HASH {
            println!("Git commit: {}", commit);
        }
        return;
    }

    match env::current_dir() {
        Ok(path) => info!("Current directory: {}", path.display()),
        Err(e) => error!("Error getting current directory: {}", e),
    };

    info!("NEEMS API v{} starting", built_info::PKG_VERSION);
    info!("Built: {}", built_info::BUILT_TIME_UTC);
    if let Some(commit) = built_info::GIT_COMMIT_HASH {
        info!("Git commit: {}", commit);
    }

    neems_api::rocket()
        .launch()
        .await
        .expect("Rocket server failed to launch");
}
