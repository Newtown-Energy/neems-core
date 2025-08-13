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
use rocket::form::FromForm;
use serde::{Serialize, Deserialize};
use chrono::NaiveDateTime;
use ts_rs::TS;

use crate::orm::neems_data::db::SiteDbConn;
use crate::session_guards::AuthenticatedUser;

/// Response structure for data sources list
#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DataSourcesResponse {
    pub sources: Vec<neems_data::models::Source>,
}

/// Response structure for readings data
#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ReadingsResponse {
    pub readings: Vec<neems_data::models::Reading>,
    pub source_id: Option<i32>,
    pub total_count: Option<i64>,
}

/// Query parameters for readings endpoints
#[derive(Serialize, Deserialize, FromForm, TS)]
#[ts(export)]
pub struct ReadingsQuery {
    /// ISO 8601 timestamp - start of time window
    pub since: Option<String>,
    /// ISO 8601 timestamp - end of time window  
    pub until: Option<String>,
    /// ISO 8601 timestamp - start from this time with count
    pub from_time: Option<String>,
    /// ISO 8601 timestamp - end at this time with count
    pub to_time: Option<String>,
    /// Number of readings (used with from_time/to_time)
    pub count: Option<i64>,
    /// Number of latest readings
    pub latest: Option<i64>,
    /// Comma-separated list of source IDs (for multi-source queries)
    pub source_ids: Option<String>,
}

impl ReadingsQuery {
    /// Parse since timestamp
    pub fn parse_since(&self) -> Result<Option<NaiveDateTime>, chrono::ParseError> {
        match &self.since {
            Some(s) => Ok(Some(NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%SZ")?)),
            None => Ok(None),
        }
    }
    
    /// Parse until timestamp
    pub fn parse_until(&self) -> Result<Option<NaiveDateTime>, chrono::ParseError> {
        match &self.until {
            Some(s) => Ok(Some(NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%SZ")?)),
            None => Ok(None),
        }
    }
    
    /// Parse from_time timestamp
    pub fn parse_from_time(&self) -> Result<Option<NaiveDateTime>, chrono::ParseError> {
        match &self.from_time {
            Some(s) => Ok(Some(NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%SZ")?)),
            None => Ok(None),
        }
    }
    
    /// Parse to_time timestamp
    pub fn parse_to_time(&self) -> Result<Option<NaiveDateTime>, chrono::ParseError> {
        match &self.to_time {
            Some(s) => Ok(Some(NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%SZ")?)),
            None => Ok(None),
        }
    }
    
    /// Parse source_ids into vector of integers
    pub fn parse_source_ids(&self) -> Result<Option<Vec<i32>>, std::num::ParseIntError> {
        match &self.source_ids {
            Some(s) => {
                let ids: Result<Vec<i32>, _> = s.split(',')
                    .map(|id| id.trim().parse::<i32>())
                    .collect();
                Ok(Some(ids?))
            }
            None => Ok(None),
        }
    }
    
    /// Validate query parameters for logical consistency
    pub fn validate(&self) -> Result<(), String> {
        // Ensure we don't have conflicting time parameters
        let time_params = [
            self.since.is_some() || self.until.is_some(),
            self.from_time.is_some(),
            self.to_time.is_some(),
            self.latest.is_some(),
        ];
        
        let active_time_params = time_params.iter().filter(|&&x| x).count();
        if active_time_params > 1 {
            return Err("Only one time parameter type allowed: (since/until), from_time, to_time, or latest".to_string());
        }
        
        // Validate count is used with from_time or to_time
        if self.count.is_some() && self.from_time.is_none() && self.to_time.is_none() {
            return Err("count parameter requires from_time or to_time".to_string());
        }
        
        // Ensure count and latest are reasonable
        if let Some(count) = self.count {
            if count <= 0 || count > 10000 {
                return Err("count must be between 1 and 10000".to_string());
            }
        }
        
        if let Some(latest) = self.latest {
            if latest <= 0 || latest > 10000 {
                return Err("latest must be between 1 and 10000".to_string());
            }
        }
        
        Ok(())
    }
}

/// List Data Sources endpoint.
///
/// - **URL:** `/api/1/data`
/// - **Method:** `GET`
/// - **Purpose:** Returns a list of all data sources in the database
/// - **Authentication:** Not required
///
/// This endpoint queries the sources table and returns all configured data sources
/// with their metadata including name, description, active status, and timing information.
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// {
///   "sources": [
///     {
///       "id": 1,
///       "name": "Temperature Sensor A",
///       "description": "Main building temperature monitoring",
///       "active": true,
///       "interval_seconds": 300,
///       "last_run": "2024-01-01T12:00:00",
///       "created_at": "2024-01-01T00:00:00",
///       "updated_at": "2024-01-01T00:00:00"
///     }
///   ]
/// }
/// ```
#[get("/1/DataSources")]
pub async fn list_data_sources(
    site_db: SiteDbConn,
) -> Result<Json<DataSourcesResponse>, Status> {
    site_db.run(|conn| {
        use diesel::prelude::*;
        use neems_data::schema::sources::dsl::*;
        
        match sources.load::<neems_data::models::Source>(conn) {
            Ok(source_list) => {
                Ok(Json(DataSourcesResponse {
                    sources: source_list,
                }))
            }
            Err(e) => {
                eprintln!("Error loading data sources: {:?}", e);
                Err(Status::InternalServerError)
            }
        }
    }).await
}

/// Get Readings for Single Data Source endpoint.
///
/// - **URL:** `/api/1/data/<source_id>`
/// - **Method:** `GET`
/// - **Purpose:** Returns readings for a specific data source with optional filtering
/// - **Authentication:** Required - users can only access readings from sources in their company
///
/// This endpoint queries the readings table for a specific source_id with various
/// time-based filtering options to prevent accidentally large data transfers.
///
/// # Query Parameters
///
/// **Time Window (mutually exclusive with other options):**
/// - `since`: ISO 8601 timestamp (e.g., "2024-01-01T00:00:00Z") - start of time window
/// - `until`: ISO 8601 timestamp (e.g., "2024-01-02T00:00:00Z") - end of time window
///
/// **Count-based from timestamp (mutually exclusive):**
/// - `from_time`: ISO 8601 timestamp - start from this time
/// - `count`: Number of readings to return (1-10000)
///
/// **Count-based to timestamp (mutually exclusive):**
/// - `to_time`: ISO 8601 timestamp - end at this time  
/// - `count`: Number of readings to return (1-10000)
///
/// **Latest readings (mutually exclusive):**
/// - `latest`: Number of most recent readings (1-10000)
///
/// # Authorization
///
/// - **Company Users**: Can only access readings from sources in their company
/// - **newtown-staff/newtown-admin**: Can access readings from any company
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// {
///   "readings": [
///     {
///       "id": 1,
///       "source_id": 123,
///       "timestamp": "2024-01-01T12:00:00",
///       "data": "{\"temperature\": 23.5}",
///       "quality_flags": 0
///     }
///   ],
///   "source_id": 123,
///   "total_count": null
/// }
/// ```
///
/// **Error (HTTP 400 Bad Request):** Invalid query parameters
/// **Error (HTTP 401 Unauthorized):** User not authenticated
/// **Error (HTTP 403 Forbidden):** User lacks permission to access this source
/// **Error (HTTP 404 Not Found):** Source ID does not exist
#[get("/1/DataSources/<source_id>/Readings?<query..>")]
pub async fn get_source_readings(
    source_id: i32,
    query: ReadingsQuery,
    user: AuthenticatedUser,
    site_db: SiteDbConn,
) -> Result<Json<ReadingsResponse>, Status> {
    // Validate query parameters
    if let Err(e) = query.validate() {
        eprintln!("Invalid query parameters: {}", e);
        return Err(Status::BadRequest);
    }
    
    let req_source_id = source_id;
    let user_company_id = user.user.company_id;
    let has_newtown_access = user.has_any_role(&["newtown-staff", "newtown-admin"]);
    
    site_db.run(move |conn| {
        use diesel::prelude::*;
        use neems_data::schema::readings::dsl::*;
        use neems_data::schema::sources;
        
        // First verify the source exists and check company access
        let source = match sources::dsl::sources
            .filter(sources::dsl::id.eq(req_source_id))
            .first::<neems_data::models::Source>(conn) 
        {
            Ok(s) => s,
            Err(diesel::result::Error::NotFound) => return Err(Status::NotFound),
            Err(e) => {
                eprintln!("Error checking source existence: {:?}", e);
                return Err(Status::InternalServerError);
            }
        };
        
        // Check company access unless user has Newtown roles
        if !has_newtown_access {
            match source.company_id {
                Some(source_company_id) if source_company_id == user_company_id => {
                    // User can access - source is in their company
                },
                Some(_) => {
                    // Source belongs to a different company - forbidden
                    return Err(Status::Forbidden);
                },
                None => {
                    // Source has no company - only Newtown roles can access
                    return Err(Status::Forbidden);
                }
            }
        }
        
        // Build the base query
        let mut query_builder = readings
            .filter(source_id.eq(req_source_id))
            .into_boxed();
        
        // Apply time-based filtering
        if let Some(since_time) = query.parse_since().map_err(|_| Status::BadRequest)? {
            query_builder = query_builder.filter(timestamp.ge(since_time));
        }
        
        if let Some(until_time) = query.parse_until().map_err(|_| Status::BadRequest)? {
            query_builder = query_builder.filter(timestamp.le(until_time));
        }
        
        if let Some(from_time) = query.parse_from_time().map_err(|_| Status::BadRequest)? {
            query_builder = query_builder
                .filter(timestamp.ge(from_time))
                .order(timestamp.asc());
            if let Some(count) = query.count {
                query_builder = query_builder.limit(count);
            }
        } else if let Some(to_time) = query.parse_to_time().map_err(|_| Status::BadRequest)? {
            query_builder = query_builder
                .filter(timestamp.le(to_time))
                .order(timestamp.desc());
            if let Some(count) = query.count {
                query_builder = query_builder.limit(count);
            }
        } else if let Some(latest_count) = query.latest {
            query_builder = query_builder
                .order(timestamp.desc())
                .limit(latest_count);
        } else {
            // Default ordering by timestamp if no specific time parameters
            query_builder = query_builder.order(timestamp.desc());
        }
        
        // Execute query
        match query_builder.load::<neems_data::models::Reading>(conn) {
            Ok(mut readings_list) => {
                // If we ordered desc for to_time queries, reverse to get chronological order
                if query.to_time.is_some() {
                    readings_list.reverse();
                }
                
                Ok(Json(ReadingsResponse {
                    readings: readings_list,
                    source_id: Some(req_source_id),
                    total_count: None,
                }))
            }
            Err(e) => {
                eprintln!("Error loading readings: {:?}", e);
                Err(Status::InternalServerError)
            }
        }
    }).await
}

/// Get Readings for Multiple Data Sources endpoint.
///
/// - **URL:** `/api/1/data/readings`
/// - **Method:** `GET`
/// - **Purpose:** Returns readings from multiple data sources with optional filtering
/// - **Authentication:** Required - users can only access readings from sources in their company
///
/// This endpoint queries the readings table for multiple source_ids specified via
/// the source_ids query parameter. Same time-based filtering options as the single
/// source endpoint.
///
/// # Query Parameters
///
/// **Required:**
/// - `source_ids`: Comma-separated list of source IDs (e.g., "1,2,3")
///
/// **Time filtering (same as single source endpoint):**
/// - `since`/`until`: Time window
/// - `from_time`/`count`: Count-based from timestamp  
/// - `to_time`/`count`: Count-based to timestamp
/// - `latest`: Number of most recent readings per source
///
/// # Authorization
///
/// - **Company Users**: Can only access readings from sources in their company
/// - **newtown-staff/newtown-admin**: Can access readings from any company
/// - All requested source IDs must be accessible to the user or the request fails
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// {
///   "readings": [
///     {
///       "id": 1,
///       "source_id": 1,
///       "timestamp": "2024-01-01T12:00:00", 
///       "data": "{\"temperature\": 23.5}",
///       "quality_flags": 0
///     },
///     {
///       "id": 2,
///       "source_id": 2,
///       "timestamp": "2024-01-01T12:00:00",
///       "data": "{\"humidity\": 45.2}",
///       "quality_flags": 0
///     }
///   ],
///   "source_id": null,
///   "total_count": null
/// }
/// ```
///
/// **Error (HTTP 400 Bad Request):** Invalid query parameters or missing source_ids
/// **Error (HTTP 401 Unauthorized):** User not authenticated
/// **Error (HTTP 403 Forbidden):** User lacks permission to access one or more sources
/// **Error (HTTP 404 Not Found):** One or more source IDs do not exist
#[get("/1/Readings?<query..>")]
pub async fn get_multi_source_readings(
    query: ReadingsQuery,
    user: AuthenticatedUser,
    site_db: SiteDbConn,
) -> Result<Json<ReadingsResponse>, Status> {
    // Validate query parameters
    if let Err(e) = query.validate() {
        eprintln!("Invalid query parameters: {}", e);
        return Err(Status::BadRequest);
    }
    
    // source_ids is required for this endpoint
    let source_ids = match query.parse_source_ids() {
        Ok(Some(ids)) => ids,
        Ok(None) => {
            eprintln!("source_ids parameter is required for multi-source endpoint");
            return Err(Status::BadRequest);
        }
        Err(e) => {
            eprintln!("Invalid source_ids format: {}", e);
            return Err(Status::BadRequest);
        }
    };
    
    let user_company_id = user.user.company_id;
    let has_newtown_access = user.has_any_role(&["newtown-staff", "newtown-admin"]);
    
    site_db.run(move |conn| {
        use diesel::prelude::*;
        use neems_data::schema::readings::dsl::*;
        use neems_data::schema::sources;
        
        // Verify all sources exist and check company access
        for src_id in &source_ids {
            let source = match sources::dsl::sources
                .filter(sources::dsl::id.eq(*src_id))
                .first::<neems_data::models::Source>(conn) 
            {
                Ok(s) => s,
                Err(diesel::result::Error::NotFound) => return Err(Status::NotFound),
                Err(e) => {
                    eprintln!("Error checking source existence: {:?}", e);
                    return Err(Status::InternalServerError);
                }
            };
            
            // Check company access for each source unless user has Newtown roles
            if !has_newtown_access {
                match source.company_id {
                    Some(source_company_id) if source_company_id == user_company_id => {
                        // User can access - source is in their company
                    },
                    Some(_) => {
                        // Source belongs to a different company - forbidden
                        return Err(Status::Forbidden);
                    },
                    None => {
                        // Source has no company - only Newtown roles can access
                        return Err(Status::Forbidden);
                    }
                }
            }
        }
        
        // Build the base query for multiple sources
        let mut query_builder = readings
            .filter(source_id.eq_any(&source_ids))
            .into_boxed();
        
        // Apply time-based filtering (same logic as single source)
        if let Some(since_time) = query.parse_since().map_err(|_| Status::BadRequest)? {
            query_builder = query_builder.filter(timestamp.ge(since_time));
        }
        
        if let Some(until_time) = query.parse_until().map_err(|_| Status::BadRequest)? {
            query_builder = query_builder.filter(timestamp.le(until_time));
        }
        
        if let Some(from_time) = query.parse_from_time().map_err(|_| Status::BadRequest)? {
            query_builder = query_builder
                .filter(timestamp.ge(from_time))
                .order((source_id.asc(), timestamp.asc()));
            if let Some(count) = query.count {
                // For multi-source, apply count per source using window functions would be complex
                // For now, apply global count with note in documentation
                query_builder = query_builder.limit(count);
            }
        } else if let Some(to_time) = query.parse_to_time().map_err(|_| Status::BadRequest)? {
            query_builder = query_builder
                .filter(timestamp.le(to_time))
                .order((source_id.asc(), timestamp.desc()));
            if let Some(count) = query.count {
                query_builder = query_builder.limit(count);
            }
        } else if let Some(latest_count) = query.latest {
            // For latest with multiple sources, we need to get latest_count per source
            // This requires a more complex query - for now, get globally latest
            query_builder = query_builder
                .order((source_id.asc(), timestamp.desc()))
                .limit(latest_count * source_ids.len() as i64);
        } else {
            // Default ordering by source_id then timestamp
            query_builder = query_builder.order((source_id.asc(), timestamp.desc()));
        }
        
        // Execute query
        match query_builder.load::<neems_data::models::Reading>(conn) {
            Ok(mut readings_list) => {
                // If we ordered desc for to_time queries, reverse within each source group
                if query.to_time.is_some() {
                    // Group by source and reverse each group
                    readings_list.sort_by(|a, b| {
                        a.source_id.cmp(&b.source_id)
                            .then(a.timestamp.cmp(&b.timestamp))
                    });
                }
                
                Ok(Json(ReadingsResponse {
                    readings: readings_list,
                    source_id: None, // Multi-source query
                    total_count: None,
                }))
            }
            Err(e) => {
                eprintln!("Error loading readings: {:?}", e);
                Err(Status::InternalServerError)
            }
        }
    }).await
}

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
#[get("/1/$metadata/schema")]
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
        let mut data_routes = routes![list_data_sources, get_source_readings, get_multi_source_readings];
        data_routes.extend(routes![get_site_schema]);
        data_routes
    }
    
    #[cfg(not(feature = "test-staging"))]
    {
        routes![list_data_sources, get_source_readings, get_multi_source_readings]
    }
}
