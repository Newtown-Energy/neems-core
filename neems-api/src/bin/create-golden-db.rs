//! create-golden-db

//! Tests were taking a long time because of db setup.  We're using sqlite for db setup, which is
//! notoriously slow on writes unless you do some work to speed it up.  I am too lazy to do that.
//! Instead, I created one testing db that can be copied as needed (i.e. once per test).  That
//! testing db is the golden database.  The fast_test_rocket function in orm/testing.rs will
//! return a rocket with a copy of the golden db attached.

//! This file is a binary that makes the goldendb.  It puts that db in target.  It names the
//! goldendb with a version number.  I'm not entirely sure what determines the version number, but
//! the goal of this is that if you change the code the version changes.

use neems_api::orm::testing::{calculate_schema_hash, create_golden_database};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Calculate the version hash
    let version_hash = calculate_schema_hash();
    let golden_db_path = PathBuf::from(format!("../target/golden_test_{}.db", version_hash));
    
    // Check if golden DB already exists
    if golden_db_path.exists() {
        println!("Golden database v{} already exists at: {:?}", version_hash, golden_db_path);
        println!("Delete it first if you want to recreate it.");
        return Ok(());
    }
    
    println!("Creating golden database v{}", version_hash);
    match create_golden_database(&version_hash) {
        Ok(path) => {
            println!("Golden database v{} created successfully at: {:?}", version_hash, path);
            println!("You can now run tests with: cargo test --features test-staging");
        },
        Err(e) => {
            eprintln!("Failed to create golden database: {}", e);
            std::process::exit(1);
        }
    }
    
    Ok(())
}
