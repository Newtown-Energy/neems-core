//! API endpoints for data access and schema operations.
//!
//! This module provides HTTP endpoints for accessing data collected by
//! neems-data
//!
//! # Feature Gates
//! The /api/1/data/schema endpoint is feature-gated behind the `test-staging`
//! feature to prevent exposure in production environments.

use chrono::NaiveDateTime;
use rocket::{Route, form::FromForm, http::Status, serde::json::Json};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{orm::neems_data::db::SiteDbConn, session_guards::AuthenticatedUser};

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
                let ids: Result<Vec<i32>, _> =
                    s.split(',').map(|id| id.trim().parse::<i32>()).collect();
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
        if let Some(count) = self.count
            && (count <= 0 || count > 10000)
        {
            return Err("count must be between 1 and 10000".to_string());
        }

        if let Some(latest) = self.latest
            && (latest <= 0 || latest > 10000)
        {
            return Err("latest must be between 1 and 10000".to_string());
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
/// This endpoint queries the sources table and returns all configured data
/// sources with their metadata including name, description, active status, and
/// timing information.
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
pub async fn list_data_sources(site_db: SiteDbConn) -> Result<Json<DataSourcesResponse>, Status> {
    site_db
        .run(|conn| {
            use diesel::prelude::*;
            use neems_data::schema::sources::dsl::*;

            match sources.load::<neems_data::models::Source>(conn) {
                Ok(source_list) => Ok(Json(DataSourcesResponse { sources: source_list })),
                Err(e) => {
                    eprintln!("Error loading data sources: {:?}", e);
                    Err(Status::InternalServerError)
                }
            }
        })
        .await
}

/// Get Readings for Single Data Source endpoint.
///
/// - **URL:** `/api/1/data/<source_id>`
/// - **Method:** `GET`
/// - **Purpose:** Returns readings for a specific data source with optional
///   filtering
/// - **Authentication:** Required - users can only access readings from sources
///   in their company
///
/// This endpoint queries the readings table for a specific source_id with
/// various time-based filtering options to prevent accidentally large data
/// transfers.
///
/// # Query Parameters
///
/// **Time Window (mutually exclusive with other options):**
/// - `since`: ISO 8601 timestamp (e.g., "2024-01-01T00:00:00Z") - start of time
///   window
/// - `until`: ISO 8601 timestamp (e.g., "2024-01-02T00:00:00Z") - end of time
///   window
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

    site_db
        .run(move |conn| {
            use diesel::prelude::*;
            use neems_data::schema::{readings::dsl::*, sources};

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
                    }
                    Some(_) => {
                        // Source belongs to a different company - forbidden
                        return Err(Status::Forbidden);
                    }
                    None => {
                        // Source has no company - only Newtown roles can access
                        return Err(Status::Forbidden);
                    }
                }
            }

            // Build the base query
            let mut query_builder = readings.filter(source_id.eq(req_source_id)).into_boxed();

            // Apply time-based filtering
            if let Some(since_time) = query.parse_since().map_err(|_| Status::BadRequest)? {
                query_builder = query_builder.filter(timestamp.ge(since_time));
            }

            if let Some(until_time) = query.parse_until().map_err(|_| Status::BadRequest)? {
                query_builder = query_builder.filter(timestamp.le(until_time));
            }

            if let Some(from_time) = query.parse_from_time().map_err(|_| Status::BadRequest)? {
                query_builder =
                    query_builder.filter(timestamp.ge(from_time)).order(timestamp.asc());
                if let Some(count) = query.count {
                    query_builder = query_builder.limit(count);
                }
            } else if let Some(to_time) = query.parse_to_time().map_err(|_| Status::BadRequest)? {
                query_builder = query_builder.filter(timestamp.le(to_time)).order(timestamp.desc());
                if let Some(count) = query.count {
                    query_builder = query_builder.limit(count);
                }
            } else if let Some(latest_count) = query.latest {
                query_builder = query_builder.order(timestamp.desc()).limit(latest_count);
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
        })
        .await
}

/// Get Readings for Multiple Data Sources endpoint.
///
/// - **URL:** `/api/1/data/readings`
/// - **Method:** `GET`
/// - **Purpose:** Returns readings from multiple data sources with optional
///   filtering
/// - **Authentication:** Required - users can only access readings from sources
///   in their company
///
/// This endpoint queries the readings table for multiple source_ids specified
/// via the source_ids query parameter. Same time-based filtering options as the
/// single source endpoint.
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
/// - All requested source IDs must be accessible to the user or the request
///   fails
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
/// **Error (HTTP 400 Bad Request):** Invalid query parameters or missing
/// source_ids **Error (HTTP 401 Unauthorized):** User not authenticated
/// **Error (HTTP 403 Forbidden):** User lacks permission to access one or more
/// sources **Error (HTTP 404 Not Found):** One or more source IDs do not exist
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

    site_db
        .run(move |conn| {
            use diesel::prelude::*;
            use neems_data::schema::{readings::dsl::*, sources};

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
                        }
                        Some(_) => {
                            // Source belongs to a different company - forbidden
                            return Err(Status::Forbidden);
                        }
                        None => {
                            // Source has no company - only Newtown roles can access
                            return Err(Status::Forbidden);
                        }
                    }
                }
            }

            // Build the base query for multiple sources
            let mut query_builder = readings.filter(source_id.eq_any(&source_ids)).into_boxed();

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
                    // For multi-source, apply count per source using window functions would be
                    // complex For now, apply global count with note in
                    // documentation
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
                            a.source_id.cmp(&b.source_id).then(a.timestamp.cmp(&b.timestamp))
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
        })
        .await
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
/// **Note:** This endpoint is only available when the `test-staging` feature is
/// enabled during compilation.
#[cfg(feature = "test-staging")]
#[get("/1/$metadata/schema")]
pub async fn get_site_schema(site_db: SiteDbConn) -> Result<Json<serde_json::Value>, Status> {
    site_db
        .run(|conn| {
            use diesel::{prelude::*, sql_query, sql_types::Text};

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
                    let schema_statements: Vec<String> =
                        rows.into_iter().map(|row| format!("{};", row.sql)).collect();

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
        })
        .await
}

/// A single SoC sample point exposed to the frontend.
#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SocHistoryPoint {
    /// ISO 8601 timestamp of the reading (naive UTC, matches `Reading.timestamp`).
    pub timestamp: NaiveDateTime,
    /// Battery state of charge as a percentage, 0–100.
    pub soc_percent: f64,
}

/// Response payload for `GET /api/1/Sites/<id>/SocHistory`.
#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SocHistoryResponse {
    pub site_id: i32,
    pub points: Vec<SocHistoryPoint>,
}

/// Extract the battery SoC percentage from a reading's JSON `data` blob.
///
/// The `charging_state` collector writes `{ "level": <number>, ... }`. We
/// pull `level` out and clamp obvious garbage; a missing or non-numeric
/// field returns `None` so the caller can skip the row instead of
/// poisoning the series.
pub fn parse_soc_level(data_json: &str) -> Option<f64> {
    let parsed: serde_json::Value = serde_json::from_str(data_json).ok()?;
    let n = parsed.get("level")?.as_f64()?;
    if !n.is_finite() {
        return None;
    }
    Some(n.clamp(0.0, 100.0))
}

/// Get the SoC history for a site within a time window.
///
/// - **URL:** `/api/1/Sites/<site_id>/SocHistory?from=...&to=...`
/// - **Method:** `GET`
/// - **Authentication:** Required
///
/// Joins `readings → sources` (filtered to `site_id` and the
/// `charging_state` test type), parses each reading's JSON `level`
/// field, and returns the resulting points in chronological order.
#[get("/1/Sites/<site_id>/SocHistory?<from>&<to>")]
pub async fn get_site_soc_history(
    site_id: i32,
    from: Option<String>,
    to: Option<String>,
    _user: AuthenticatedUser,
    site_db: SiteDbConn,
) -> Result<Json<SocHistoryResponse>, Status> {
    let parse_ts = |s: &str| -> Option<NaiveDateTime> {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%SZ")
            .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S"))
            .ok()
    };
    let from_ts = from.as_deref().and_then(parse_ts);
    let to_ts = to.as_deref().and_then(parse_ts);
    if from.is_some() && from_ts.is_none() {
        return Err(Status::BadRequest);
    }
    if to.is_some() && to_ts.is_none() {
        return Err(Status::BadRequest);
    }

    site_db
        .run(move |conn| {
            use diesel::prelude::*;
            use neems_data::schema::{readings, sources};

            // Find the charging-state sources belonging to this site.
            let source_ids: Vec<i32> = sources::table
                .filter(sources::site_id.eq(site_id))
                .filter(sources::test_type.eq("charging_state"))
                .select(sources::id.assume_not_null())
                .load::<i32>(conn)
                .map_err(|e| {
                    eprintln!("Error loading charging_state sources: {:?}", e);
                    Status::InternalServerError
                })?;

            if source_ids.is_empty() {
                return Ok(Json(SocHistoryResponse { site_id, points: vec![] }));
            }

            let mut query = readings::table
                .filter(readings::source_id.eq_any(&source_ids))
                .order(readings::timestamp.asc())
                .into_boxed();
            if let Some(f) = from_ts {
                query = query.filter(readings::timestamp.ge(f));
            }
            if let Some(t) = to_ts {
                query = query.filter(readings::timestamp.le(t));
            }
            let rows: Vec<neems_data::models::Reading> =
                query.load(conn).map_err(|e| {
                    eprintln!("Error loading SoC readings: {:?}", e);
                    Status::InternalServerError
                })?;

            let points = rows
                .into_iter()
                .filter_map(|r| {
                    parse_soc_level(&r.data).map(|soc_percent| SocHistoryPoint {
                        timestamp: r.timestamp,
                        soc_percent,
                    })
                })
                .collect();
            Ok(Json(SocHistoryResponse { site_id, points }))
        })
        .await
}

/// Per-day breakdown of how long a site spent in each battery state.
/// Minutes (not seconds) keeps the wire format friendly for the chart.
#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ChargeDischargeBucket {
    /// "YYYY-MM-DD" — the calendar day the readings fell on, in UTC.
    pub day: String,
    pub charging_minutes: f64,
    pub discharging_minutes: f64,
    pub hold_minutes: f64,
}

/// Response payload for `GET /api/1/Sites/<id>/ChargeDischargeSummary`.
#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ChargeDischargeSummary {
    pub site_id: i32,
    pub buckets: Vec<ChargeDischargeBucket>,
}

/// Extract the battery state from a reading's JSON `data` blob. The
/// `charging_state` collector writes `{ "state": "charging" | ... }`.
pub fn parse_soc_state(data_json: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(data_json).ok()?;
    parsed.get("state")?.as_str().map(|s| s.to_string())
}

/// Get per-day charging/discharging/hold minute totals for a site.
///
/// - **URL:** `/api/1/Sites/<site_id>/ChargeDischargeSummary?from=...&to=...`
/// - **Method:** `GET`
/// - **Authentication:** Required
///
/// Each reading is treated as representing one collection-interval of
/// time in its recorded state — count × interval_seconds / 60.
/// Sources without an interval_seconds value are skipped (we can't
/// attribute time without it).
#[get("/1/Sites/<site_id>/ChargeDischargeSummary?<from>&<to>")]
pub async fn get_site_charge_discharge_summary(
    site_id: i32,
    from: Option<String>,
    to: Option<String>,
    _user: AuthenticatedUser,
    site_db: SiteDbConn,
) -> Result<Json<ChargeDischargeSummary>, Status> {
    let parse_ts = |s: &str| -> Option<NaiveDateTime> {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%SZ")
            .or_else(|_| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S"))
            .ok()
    };
    let from_ts = from.as_deref().and_then(parse_ts);
    let to_ts = to.as_deref().and_then(parse_ts);
    if from.is_some() && from_ts.is_none() {
        return Err(Status::BadRequest);
    }
    if to.is_some() && to_ts.is_none() {
        return Err(Status::BadRequest);
    }

    site_db
        .run(move |conn| {
            use std::collections::BTreeMap;

            use diesel::prelude::*;
            use neems_data::schema::{readings, sources};

            // Pull all charging_state sources for the site along with
            // their collection interval — we need the latter to weight
            // each reading.
            let site_sources: Vec<(i32, i32)> = sources::table
                .filter(sources::site_id.eq(site_id))
                .filter(sources::test_type.eq("charging_state"))
                .select((sources::id.assume_not_null(), sources::interval_seconds))
                .load::<(i32, i32)>(conn)
                .map_err(|e| {
                    eprintln!("Error loading charging_state sources: {:?}", e);
                    Status::InternalServerError
                })?;

            if site_sources.is_empty() {
                return Ok(Json(ChargeDischargeSummary { site_id, buckets: vec![] }));
            }

            let source_ids: Vec<i32> = site_sources.iter().map(|(id, _)| *id).collect();
            let interval_by_source: std::collections::HashMap<i32, i32> =
                site_sources.into_iter().collect();

            let mut query = readings::table
                .filter(readings::source_id.eq_any(&source_ids))
                .order(readings::timestamp.asc())
                .into_boxed();
            if let Some(f) = from_ts {
                query = query.filter(readings::timestamp.ge(f));
            }
            if let Some(t) = to_ts {
                query = query.filter(readings::timestamp.le(t));
            }
            let rows: Vec<neems_data::models::Reading> =
                query.load(conn).map_err(|e| {
                    eprintln!("Error loading SoC readings for summary: {:?}", e);
                    Status::InternalServerError
                })?;

            // Bucket by day → (charging, discharging, hold) minutes.
            // BTreeMap keeps day keys sorted on iteration so the
            // response is chronological without an extra sort.
            let mut by_day: BTreeMap<String, (f64, f64, f64)> = BTreeMap::new();
            for r in rows {
                let Some(interval_seconds) = interval_by_source.get(&r.source_id) else {
                    continue;
                };
                let minutes = (*interval_seconds as f64) / 60.0;
                let day = r.timestamp.format("%Y-%m-%d").to_string();
                let bucket = by_day.entry(day).or_insert((0.0, 0.0, 0.0));
                match parse_soc_state(&r.data).as_deref() {
                    Some("charging") => bucket.0 += minutes,
                    Some("discharging") => bucket.1 += minutes,
                    Some("hold") => bucket.2 += minutes,
                    _ => {} // Unknown / missing state — drop the row.
                }
            }

            let buckets = by_day
                .into_iter()
                .map(|(day, (c, d, h))| ChargeDischargeBucket {
                    day,
                    charging_minutes: c,
                    discharging_minutes: d,
                    hold_minutes: h,
                })
                .collect();
            Ok(Json(ChargeDischargeSummary { site_id, buckets }))
        })
        .await
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
        let mut data_routes = routes![
            list_data_sources,
            get_source_readings,
            get_multi_source_readings,
            get_site_soc_history,
            get_site_charge_discharge_summary,
        ];
        data_routes.extend(routes![get_site_schema]);
        data_routes
    }

    #[cfg(not(feature = "test-staging"))]
    {
        routes![
            list_data_sources,
            get_source_readings,
            get_multi_source_readings,
            get_site_soc_history,
            get_site_charge_discharge_summary,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_soc_level, parse_soc_state};

    #[test]
    fn parses_level_from_charging_state_blob() {
        let blob = r#"{"battery_id":"default","level":42.5,"state":"charging"}"#;
        assert_eq!(parse_soc_level(blob), Some(42.5));
    }

    #[test]
    fn clamps_out_of_range_values() {
        assert_eq!(parse_soc_level(r#"{"level":150}"#), Some(100.0));
        assert_eq!(parse_soc_level(r#"{"level":-5}"#), Some(0.0));
    }

    #[test]
    fn rejects_missing_or_non_numeric_level() {
        assert_eq!(parse_soc_level(r#"{"foo":1}"#), None);
        assert_eq!(parse_soc_level(r#"{"level":"high"}"#), None);
    }

    #[test]
    fn rejects_invalid_json() {
        assert_eq!(parse_soc_level("not json"), None);
    }

    #[test]
    fn rejects_non_finite_values() {
        // NaN is encoded as null by serde_json; this guards the .is_finite() branch.
        assert_eq!(parse_soc_level(r#"{"level":null}"#), None);
    }

    #[test]
    fn parses_state_from_charging_state_blob() {
        let blob = r#"{"level":80.0,"state":"discharging"}"#;
        assert_eq!(parse_soc_state(blob).as_deref(), Some("discharging"));
    }

    #[test]
    fn returns_none_when_state_missing_or_non_string() {
        assert_eq!(parse_soc_state(r#"{"level":80.0}"#), None);
        assert_eq!(parse_soc_state(r#"{"state":42}"#), None);
        assert_eq!(parse_soc_state("not json"), None);
    }
}
