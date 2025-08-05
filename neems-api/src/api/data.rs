//! API endpoints for data access and schema operations.
//!
//! This module provides HTTP endpoints for accessing data collected by neems-data 
//!
//! # Feature Gates
//! The /api/1/data/schema endpoint is feature-gated behind the `test-staging` feature
//! to prevent exposure in production environments.

use rocket::Route;
use rocket::http::Status;
use rocket::serde::json::Json;

#[cfg(feature = "test-staging")]
use crate::orm::neems_data::db::SiteDbConn;

/// Get Site Database Schema endpoint.
///
/// - **URL:** `/api/1/data/schema`
/// - **Method:** `GET`
/// - **Purpose:** Returns the SQLite database schema as JSON
/// - **Authentication:** Not required
/// - **Feature Gate:** Only available with `test-staging` feature
///
/// This endpoint dumps the complete schema of the site database by querying
/// SQLite's metadata tables and returning the CREATE statements as JSON.
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// {
///   "schema": "CREATE TABLE users (...); CREATE TABLE sites (...);"
/// }
/// ```
///
/// **Note:** This endpoint is only available when the `test-staging` feature is enabled during compilation.
#[cfg(feature = "test-staging")]
#[get("/1/data/schema")]
pub async fn get_site_schema(
    site_db: SiteDbConn,
) -> Result<Json<serde_json::Value>, Status> {
    site_db.run(|conn| {
        use diesel::prelude::*;
        use diesel::sql_query;
        use diesel::sql_types::Text;
        
        #[derive(QueryableByName)]
        struct SchemaRow {
            #[diesel(sql_type = Text)]
            sql: String,
        }
        
        // Query sqlite_master to get all CREATE statements
        let schema_query = "
            SELECT sql 
            FROM sqlite_master 
            WHERE type IN ('table', 'index', 'trigger', 'view') 
            AND name NOT LIKE 'sqlite_%'
            AND sql IS NOT NULL
            ORDER BY 
                CASE type 
                    WHEN 'table' THEN 1 
                    WHEN 'index' THEN 2 
                    WHEN 'trigger' THEN 3 
                    WHEN 'view' THEN 4 
                END, name
        ";
        
        match sql_query(schema_query).load::<SchemaRow>(conn) {
            Ok(rows) => {
                let schema_statements: Vec<String> = rows
                    .into_iter()
                    .map(|row| format!("{};", row.sql))
                    .collect();
                
                let full_schema = schema_statements.join("\n");
                
                Ok(Json(serde_json::json!({
                    "schema": full_schema
                })))
            }
            Err(e) => {
                eprintln!("Error getting database schema: {:?}", e);
                Err(Status::InternalServerError)
            }
        }
    }).await
}

/// Returns a vector of all routes defined in this module.
///
/// This function collects all the route handlers defined in this module
/// and returns them as a vector for registration with the Rocket framework.
///
/// # Returns
/// A vector containing all route handlers for data endpoints
pub fn routes() -> Vec<Route> {
    #[cfg(feature = "test-staging")]
    {
        routes![get_site_schema]
    }
    
    #[cfg(not(feature = "test-staging"))]
    {
        vec![]
    }
}
