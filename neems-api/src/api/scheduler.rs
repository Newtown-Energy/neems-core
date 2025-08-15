//! API endpoints for managing scheduler scripts and overrides.
//!
//! This module provides HTTP endpoints for creating, updating, and managing
//! scheduler scripts and overrides for site state management.

use rocket::Route;
use rocket::http::Status;
use rocket::response::{self, status};
use rocket::serde::json::Json;
use serde::{Serialize, Deserialize};
use ts_rs::TS;
use chrono::NaiveDateTime;

use crate::logged_json::LoggedJson;
use crate::models::{
    SchedulerScript, SchedulerScriptInput, UpdateSchedulerScriptRequest,
    SchedulerOverride, SchedulerOverrideInput,
    SiteState
};
use crate::odata_query::{ODataQuery, ODataCollectionResponse, build_context_url, apply_select};
use crate::orm::DbConn;
use crate::orm::scheduler_script::{
    get_all_scheduler_scripts, get_scheduler_script_by_id, insert_scheduler_script,
    update_scheduler_script, delete_scheduler_script, get_scheduler_scripts_by_site,
    is_script_name_unique_in_site
};
use crate::orm::scheduler_override::{
    get_all_scheduler_overrides, insert_scheduler_override, get_scheduler_overrides_by_site,
    check_override_conflicts
};
use crate::orm::scheduler::{get_site_state_at_datetime, execute_scheduler_for_site, SchedulerService};
use crate::session_guards::AuthenticatedUser;

/// Error response structure for scheduler API failures.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct ErrorResponse {
    pub error: String,
}

/// Request for validating a script.
#[derive(Deserialize, TS)]
#[ts(export)]
pub struct ValidateScriptRequest {
    pub script_content: String,
    pub language: Option<String>,
    pub site_id: i32,
}

/// Response for script validation.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct ValidateScriptResponse {
    pub is_valid: bool,
    pub error: Option<String>,
    pub test_state: Option<String>,
    pub execution_time_ms: Option<u64>,
}

/// Request for executing scheduler for a site.
#[derive(Deserialize, TS)]
#[ts(export)]
pub struct ExecuteSchedulerRequest {
    pub site_id: i32,
    pub datetime: Option<NaiveDateTime>,
}

/// Response for scheduler execution.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct ExecuteSchedulerResponse {
    pub state: String,
    pub source: String,
    pub execution_time_ms: u64,
    pub error: Option<String>,
}

/// Response for site state query.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct SiteStateResponse {
    pub site_id: i32,
    pub state: String,
    pub datetime: NaiveDateTime,
    pub source: String,
    pub execution_time_ms: u64,
    pub error: Option<String>,
}

// ========== SCHEDULER SCRIPT ENDPOINTS ==========

/// Create SchedulerScript endpoint.
///
/// - **URL:** `/api/1/SchedulerScripts`
/// - **Method:** `POST`
/// - **Purpose:** Creates a new scheduler script
/// - **Authentication:** Required
/// - **Authorization:** Company admins, newtown-admin, newtown-staff
#[post("/1/SchedulerScripts", data = "<new_script>")]
pub async fn create_scheduler_script(
    db: DbConn,
    new_script: LoggedJson<SchedulerScriptInput>,
    auth_user: AuthenticatedUser,
) -> Result<status::Created<Json<SchedulerScript>>, response::status::Custom<Json<ErrorResponse>>> {
    // Check authorization
    if !auth_user.has_any_role(&["newtown-admin", "newtown-staff", "admin"]) {
        let err = Json(ErrorResponse {
            error: "Insufficient permissions to create scheduler scripts".to_string(),
        });
        return Err(response::status::Custom(Status::Forbidden, err));
    }

    db.run(move |conn| {
        let script_input = new_script.into_inner();

        // Validate that the script name is unique for this site
        match is_script_name_unique_in_site(conn, script_input.site_id, &script_input.name, None) {
            Ok(true) => {} // Name is unique, continue
            Ok(false) => {
                let err = Json(ErrorResponse {
                    error: format!("Script name '{}' already exists for this site", script_input.name),
                });
                return Err(response::status::Custom(Status::Conflict, err));
            }
            Err(e) => {
                eprintln!("Error checking script name uniqueness: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Database error while validating script name".to_string(),
                });
                return Err(response::status::Custom(Status::InternalServerError, err));
            }
        }

        // Create the script
        match insert_scheduler_script(conn, script_input, Some(auth_user.user.id)) {
            Ok(script) => Ok(status::Created::new("/").body(Json(script))),
            Err(e) => {
                eprintln!("Error creating scheduler script: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Database error while creating scheduler script".to_string(),
                });
                Err(response::status::Custom(Status::InternalServerError, err))
            }
        }
    })
    .await
}

/// List SchedulerScripts endpoint.
///
/// - **URL:** `/api/1/SchedulerScripts`
/// - **Method:** `GET`
/// - **Purpose:** Retrieves all scheduler scripts
/// - **Authentication:** Required
/// - **Authorization:** Company admins can see scripts for their sites, newtown staff can see all
#[get("/1/SchedulerScripts?<query..>")]
pub async fn list_scheduler_scripts(
    db: DbConn,
    _auth_user: AuthenticatedUser,
    query: ODataQuery,
) -> Result<Json<serde_json::Value>, Status> {
    // Validate query options
    query.validate().map_err(|_| Status::BadRequest)?;

    let scripts = db.run(|conn| {
        // For now, get all scripts. In production, you might want to filter by company access
        get_all_scheduler_scripts(conn).map_err(|e| {
            eprintln!("Error listing scheduler scripts: {:?}", e);
            Status::InternalServerError
        })
    })
    .await?;

    // TODO: Apply filtering based on user permissions (company-based access)
    let mut filtered_scripts = scripts;

    // Apply OData filtering if specified
    if let Some(filter_expr) = query.parse_filter() {
        filtered_scripts = filtered_scripts
            .into_iter()
            .filter(|script| {
                match &filter_expr.property.as_str() {
                    &"name" => match &filter_expr.value {
                        crate::odata_query::FilterValue::String(s) => match filter_expr.operator {
                            crate::odata_query::FilterOperator::Eq => script.name == *s,
                            crate::odata_query::FilterOperator::Ne => script.name != *s,
                            crate::odata_query::FilterOperator::Contains => script.name.contains(s),
                            _ => true,
                        },
                        _ => true,
                    },
                    &"is_active" => match &filter_expr.value {
                        crate::odata_query::FilterValue::Boolean(b) => script.is_active == *b,
                        _ => true,
                    },
                    _ => true, // Unknown property, don't filter
                }
            })
            .collect();
    }

    // Apply sorting if specified
    if let Some(orderby) = query.parse_orderby() {
        for (property, direction) in orderby.iter().rev() {
            match property.as_str() {
                "name" => {
                    filtered_scripts.sort_by(|a, b| {
                        let cmp = a.name.cmp(&b.name);
                        match direction {
                            crate::odata_query::OrderDirection::Asc => cmp,
                            crate::odata_query::OrderDirection::Desc => cmp.reverse(),
                        }
                    });
                }
                "id" => {
                    filtered_scripts.sort_by(|a, b| {
                        let cmp = a.id.cmp(&b.id);
                        match direction {
                            crate::odata_query::OrderDirection::Asc => cmp,
                            crate::odata_query::OrderDirection::Desc => cmp.reverse(),
                        }
                    });
                }
                _ => {} // Unknown property, don't sort
            }
        }
    }

    // Get count before applying top/skip
    let total_count = filtered_scripts.len() as i64;

    // Apply skip and top
    if let Some(skip) = query.skip {
        filtered_scripts = filtered_scripts.into_iter().skip(skip as usize).collect();
    }
    if let Some(top) = query.top {
        filtered_scripts = filtered_scripts.into_iter().take(top as usize).collect();
    }

    // Apply $select if specified
    let select_props = query.parse_select();
    let selected_scripts: Result<Vec<serde_json::Value>, _> = filtered_scripts
        .iter()
        .map(|script| {
            let script_json = serde_json::to_value(script).map_err(|_| Status::InternalServerError)?;
            apply_select(&script_json, select_props.as_deref()).map_err(|_| Status::InternalServerError)
        })
        .collect();

    let selected_scripts = selected_scripts.map_err(|_| Status::InternalServerError)?;

    // Build OData response
    let context = build_context_url("http://localhost/api/1", "SchedulerScripts", select_props.as_deref());
    let mut response = ODataCollectionResponse::new(context, selected_scripts);

    // Add count if requested
    if query.count.unwrap_or(false) {
        response = response.with_count(total_count);
    }

    Ok(Json(serde_json::to_value(response).map_err(|_| Status::InternalServerError)?))
}

/// Get SchedulerScript endpoint.
///
/// - **URL:** `/api/1/SchedulerScripts/<script_id>`
/// - **Method:** `GET`
/// - **Purpose:** Retrieves a specific scheduler script by ID
/// - **Authentication:** Required
#[get("/1/SchedulerScripts/<script_id>")]
pub async fn get_scheduler_script(
    db: DbConn,
    script_id: i32,
    _auth_user: AuthenticatedUser,
) -> Result<Json<SchedulerScript>, response::status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        match get_scheduler_script_by_id(conn, script_id) {
            Ok(Some(script)) => Ok(Json(script)),
            Ok(None) => {
                let err = Json(ErrorResponse {
                    error: "Scheduler script not found".to_string(),
                });
                Err(response::status::Custom(Status::NotFound, err))
            }
            Err(e) => {
                eprintln!("Error getting scheduler script: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Database error while retrieving scheduler script".to_string(),
                });
                Err(response::status::Custom(Status::InternalServerError, err))
            }
        }
    })
    .await
}

/// Update SchedulerScript endpoint.
///
/// - **URL:** `/api/1/SchedulerScripts/<script_id>`
/// - **Method:** `PUT`
/// - **Purpose:** Updates a scheduler script
/// - **Authentication:** Required
/// - **Authorization:** Company admins, newtown-admin, newtown-staff
#[put("/1/SchedulerScripts/<script_id>", data = "<update_request>")]
pub async fn update_scheduler_script_endpoint(
    db: DbConn,
    script_id: i32,
    update_request: Json<UpdateSchedulerScriptRequest>,
    auth_user: AuthenticatedUser,
) -> Result<Json<SchedulerScript>, response::status::Custom<Json<ErrorResponse>>> {
    // Check authorization
    if !auth_user.has_any_role(&["newtown-admin", "newtown-staff", "admin"]) {
        let err = Json(ErrorResponse {
            error: "Insufficient permissions to update scheduler scripts".to_string(),
        });
        return Err(response::status::Custom(Status::Forbidden, err));
    }

    db.run(move |conn| {
        match update_scheduler_script(conn, script_id, update_request.into_inner(), Some(auth_user.user.id)) {
            Ok(script) => Ok(Json(script)),
            Err(diesel::result::Error::NotFound) => {
                let err = Json(ErrorResponse {
                    error: "Scheduler script not found".to_string(),
                });
                Err(response::status::Custom(Status::NotFound, err))
            }
            Err(e) => {
                eprintln!("Error updating scheduler script: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Database error while updating scheduler script".to_string(),
                });
                Err(response::status::Custom(Status::InternalServerError, err))
            }
        }
    })
    .await
}

/// Delete SchedulerScript endpoint.
///
/// - **URL:** `/api/1/SchedulerScripts/<script_id>`
/// - **Method:** `DELETE`
/// - **Purpose:** Deletes a scheduler script
/// - **Authentication:** Required
/// - **Authorization:** Company admins, newtown-admin, newtown-staff
#[delete("/1/SchedulerScripts/<script_id>")]
pub async fn delete_scheduler_script_endpoint(
    db: DbConn,
    script_id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Status, response::status::Custom<Json<ErrorResponse>>> {
    // Check authorization
    if !auth_user.has_any_role(&["newtown-admin", "newtown-staff", "admin"]) {
        let err = Json(ErrorResponse {
            error: "Insufficient permissions to delete scheduler scripts".to_string(),
        });
        return Err(response::status::Custom(Status::Forbidden, err));
    }

    db.run(move |conn| {
        match delete_scheduler_script(conn, script_id, Some(auth_user.user.id)) {
            Ok(true) => Ok(Status::NoContent),
            Ok(false) => {
                let err = Json(ErrorResponse {
                    error: "Scheduler script not found".to_string(),
                });
                Err(response::status::Custom(Status::NotFound, err))
            }
            Err(e) => {
                eprintln!("Error deleting scheduler script: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Database error while deleting scheduler script".to_string(),
                });
                Err(response::status::Custom(Status::InternalServerError, err))
            }
        }
    })
    .await
}

// ========== SCHEDULER OVERRIDE ENDPOINTS ==========

/// Create SchedulerOverride endpoint.
///
/// - **URL:** `/api/1/SchedulerOverrides`
/// - **Method:** `POST`
/// - **Purpose:** Creates a new scheduler override
/// - **Authentication:** Required
/// - **Authorization:** Company admins, newtown-admin, newtown-staff
#[post("/1/SchedulerOverrides", data = "<new_override>")]
pub async fn create_scheduler_override(
    db: DbConn,
    new_override: LoggedJson<SchedulerOverrideInput>,
    auth_user: AuthenticatedUser,
) -> Result<status::Created<Json<SchedulerOverride>>, response::status::Custom<Json<ErrorResponse>>> {
    // Check authorization
    if !auth_user.has_any_role(&["newtown-admin", "newtown-staff", "admin"]) {
        let err = Json(ErrorResponse {
            error: "Insufficient permissions to create scheduler overrides".to_string(),
        });
        return Err(response::status::Custom(Status::Forbidden, err));
    }

    db.run(move |conn| {
        let override_input = new_override.into_inner();

        // Validate state value
        if let Err(e) = SiteState::from_str(&override_input.state) {
            let err = Json(ErrorResponse {
                error: format!("Invalid state value: {}", e),
            });
            return Err(response::status::Custom(Status::BadRequest, err));
        }

        // Validate time range
        if override_input.end_time <= override_input.start_time {
            let err = Json(ErrorResponse {
                error: "End time must be after start time".to_string(),
            });
            return Err(response::status::Custom(Status::BadRequest, err));
        }

        // Check for conflicts with existing overrides
        match check_override_conflicts(
            conn,
            override_input.site_id,
            override_input.start_time,
            override_input.end_time,
            None,
        ) {
            Ok(conflicts) => {
                if !conflicts.is_empty() {
                    let err = Json(ErrorResponse {
                        error: format!("Override conflicts with {} existing override(s)", conflicts.len()),
                    });
                    return Err(response::status::Custom(Status::Conflict, err));
                }
            }
            Err(e) => {
                eprintln!("Error checking override conflicts: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Database error while checking for conflicts".to_string(),
                });
                return Err(response::status::Custom(Status::InternalServerError, err));
            }
        }

        // Create the override
        match insert_scheduler_override(conn, override_input, auth_user.user.id, Some(auth_user.user.id)) {
            Ok(override_record) => Ok(status::Created::new("/").body(Json(override_record))),
            Err(e) => {
                eprintln!("Error creating scheduler override: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Database error while creating scheduler override".to_string(),
                });
                Err(response::status::Custom(Status::InternalServerError, err))
            }
        }
    })
    .await
}

/// List SchedulerOverrides endpoint.
///
/// - **URL:** `/api/1/SchedulerOverrides`
/// - **Method:** `GET`
/// - **Purpose:** Retrieves all scheduler overrides
/// - **Authentication:** Required
#[get("/1/SchedulerOverrides?<query..>")]
pub async fn list_scheduler_overrides(
    db: DbConn,
    _auth_user: AuthenticatedUser,
    query: ODataQuery,
) -> Result<Json<serde_json::Value>, Status> {
    // Validate query options
    query.validate().map_err(|_| Status::BadRequest)?;

    let overrides = db.run(|conn| {
        get_all_scheduler_overrides(conn).map_err(|e| {
            eprintln!("Error listing scheduler overrides: {:?}", e);
            Status::InternalServerError
        })
    })
    .await?;

    // Apply OData filtering, sorting, etc. (similar to scripts)
    let mut filtered_overrides = overrides;

    // Get count before applying top/skip
    let total_count = filtered_overrides.len() as i64;

    // Apply skip and top
    if let Some(skip) = query.skip {
        filtered_overrides = filtered_overrides.into_iter().skip(skip as usize).collect();
    }
    if let Some(top) = query.top {
        filtered_overrides = filtered_overrides.into_iter().take(top as usize).collect();
    }

    // Apply $select if specified
    let select_props = query.parse_select();
    let selected_overrides: Result<Vec<serde_json::Value>, _> = filtered_overrides
        .iter()
        .map(|override_record| {
            let override_json = serde_json::to_value(override_record).map_err(|_| Status::InternalServerError)?;
            apply_select(&override_json, select_props.as_deref()).map_err(|_| Status::InternalServerError)
        })
        .collect();

    let selected_overrides = selected_overrides.map_err(|_| Status::InternalServerError)?;

    // Build OData response
    let context = build_context_url("http://localhost/api/1", "SchedulerOverrides", select_props.as_deref());
    let mut response = ODataCollectionResponse::new(context, selected_overrides);

    // Add count if requested
    if query.count.unwrap_or(false) {
        response = response.with_count(total_count);
    }

    Ok(Json(serde_json::to_value(response).map_err(|_| Status::InternalServerError)?))
}

// ========== NAVIGATION PROPERTY ENDPOINTS ==========

/// Get Site SchedulerScripts Navigation endpoint.
///
/// - **URL:** `/api/1/Sites/<site_id>/SchedulerScripts`
/// - **Method:** `GET`
/// - **Purpose:** Retrieves scheduler scripts for a specific site
/// - **Authentication:** Required
#[get("/1/Sites/<site_id>/SchedulerScripts")]
pub async fn get_site_scheduler_scripts(
    db: DbConn,
    site_id: i32,
    _auth_user: AuthenticatedUser,
) -> Result<Json<Vec<SchedulerScript>>, Status> {
    db.run(move |conn| {
        get_scheduler_scripts_by_site(conn, site_id)
            .map(Json)
            .map_err(|_| Status::InternalServerError)
    })
    .await
}

/// Get Site SchedulerOverrides Navigation endpoint.
///
/// - **URL:** `/api/1/Sites/<site_id>/SchedulerOverrides`
/// - **Method:** `GET`
/// - **Purpose:** Retrieves scheduler overrides for a specific site
/// - **Authentication:** Required
#[get("/1/Sites/<site_id>/SchedulerOverrides")]
pub async fn get_site_scheduler_overrides(
    db: DbConn,
    site_id: i32,
    _auth_user: AuthenticatedUser,
) -> Result<Json<Vec<SchedulerOverride>>, Status> {
    db.run(move |conn| {
        get_scheduler_overrides_by_site(conn, site_id)
            .map(Json)
            .map_err(|_| Status::InternalServerError)
    })
    .await
}

// ========== CUSTOM ACTION ENDPOINTS ==========

/// Validate SchedulerScript action.
///
/// - **URL:** `/api/1/SchedulerScripts/<script_id>/validate`
/// - **Method:** `POST`
/// - **Purpose:** Validates a scheduler script
/// - **Authentication:** Required
#[post("/1/SchedulerScripts/<script_id>/validate")]
pub async fn validate_scheduler_script(
    db: DbConn,
    script_id: i32,
    _auth_user: AuthenticatedUser,
) -> Result<Json<ValidateScriptResponse>, response::status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        // Get the script
        let script = match get_scheduler_script_by_id(conn, script_id) {
            Ok(Some(script)) => script,
            Ok(None) => {
                let err = Json(ErrorResponse {
                    error: "Scheduler script not found".to_string(),
                });
                return Err(response::status::Custom(Status::NotFound, err));
            }
            Err(e) => {
                eprintln!("Error getting scheduler script: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Database error while retrieving scheduler script".to_string(),
                });
                return Err(response::status::Custom(Status::InternalServerError, err));
            }
        };

        // Validate the script
        match SchedulerService::new() {
            Ok(service) => {
                match service.validate_script(conn, &script, script.site_id) {
                    Ok(validation_result) => {
                        let response = ValidateScriptResponse {
                            is_valid: validation_result.is_valid,
                            error: validation_result.error,
                            test_state: validation_result.test_execution.as_ref().map(|r| r.state.as_str().to_string()),
                            execution_time_ms: validation_result.test_execution.map(|r| r.execution_time_ms),
                        };
                        Ok(Json(response))
                    }
                    Err(e) => {
                        let err = Json(ErrorResponse {
                            error: format!("Validation error: {}", e),
                        });
                        Err(response::status::Custom(Status::InternalServerError, err))
                    }
                }
            }
            Err(e) => {
                let err = Json(ErrorResponse {
                    error: format!("Failed to create scheduler service: {}", e),
                });
                Err(response::status::Custom(Status::InternalServerError, err))
            }
        }
    })
    .await
}

/// Execute scheduler for site action.
///
/// - **URL:** `/api/1/Sites/<site_id>/scheduler/execute`
/// - **Method:** `POST`
/// - **Purpose:** Executes the scheduler for a specific site
/// - **Authentication:** Required
#[post("/1/Sites/<site_id>/scheduler/execute", data = "<execute_request>")]
pub async fn execute_site_scheduler(
    db: DbConn,
    site_id: i32,
    execute_request: Option<Json<ExecuteSchedulerRequest>>,
    _auth_user: AuthenticatedUser,
) -> Result<Json<ExecuteSchedulerResponse>, response::status::Custom<Json<ErrorResponse>>> {
    let datetime = execute_request
        .and_then(|req| req.datetime)
        .or_else(|| Some(chrono::Utc::now().naive_utc()));

    db.run(move |conn| {
        match execute_scheduler_for_site(conn, site_id, datetime) {
            Ok(result) => {
                let response = ExecuteSchedulerResponse {
                    state: result.state.as_str().to_string(),
                    source: match result.source {
                        crate::orm::scheduler::StateSource::Override(id) => format!("override:{}", id),
                        crate::orm::scheduler::StateSource::Script(id) => format!("script:{}", id),
                        crate::orm::scheduler::StateSource::Default => "default".to_string(),
                    },
                    execution_time_ms: result.execution_time_ms,
                    error: result.error,
                };
                Ok(Json(response))
            }
            Err(e) => {
                let err = Json(ErrorResponse {
                    error: format!("Scheduler execution error: {}", e),
                });
                Err(response::status::Custom(Status::InternalServerError, err))
            }
        }
    })
    .await
}

/// Get site state at specific datetime.
///
/// - **URL:** `/api/1/Sites/<site_id>/scheduler/state?datetime=<iso8601>`
/// - **Method:** `GET`
/// - **Purpose:** Gets the state for a site at a specific datetime
/// - **Authentication:** Required
#[get("/1/Sites/<site_id>/scheduler/state?<datetime>")]
pub async fn get_site_state(
    db: DbConn,
    site_id: i32,
    datetime: Option<String>,
    _auth_user: AuthenticatedUser,
) -> Result<Json<SiteStateResponse>, response::status::Custom<Json<ErrorResponse>>> {
    // Parse datetime or use current time
    let query_datetime = if let Some(dt_str) = datetime {
        match chrono::NaiveDateTime::parse_from_str(&dt_str, "%Y-%m-%dT%H:%M:%S") {
            Ok(dt) => dt,
            Err(_) => {
                let err = Json(ErrorResponse {
                    error: "Invalid datetime format. Use ISO 8601 format: YYYY-MM-DDTHH:MM:SS".to_string(),
                });
                return Err(response::status::Custom(Status::BadRequest, err));
            }
        }
    } else {
        chrono::Utc::now().naive_utc()
    };

    db.run(move |conn| {
        match get_site_state_at_datetime(conn, site_id, query_datetime) {
            Ok(result) => {
                let response = SiteStateResponse {
                    site_id,
                    state: result.state.as_str().to_string(),
                    datetime: query_datetime,
                    source: match result.source {
                        crate::orm::scheduler::StateSource::Override(id) => format!("override:{}", id),
                        crate::orm::scheduler::StateSource::Script(id) => format!("script:{}", id),
                        crate::orm::scheduler::StateSource::Default => "default".to_string(),
                    },
                    execution_time_ms: result.execution_time_ms,
                    error: result.error,
                };
                Ok(Json(response))
            }
            Err(e) => {
                let err = Json(ErrorResponse {
                    error: format!("Error getting site state: {}", e),
                });
                Err(response::status::Custom(Status::InternalServerError, err))
            }
        }
    })
    .await
}

/// Returns a vector of all routes defined in this module.
pub fn routes() -> Vec<Route> {
    routes![
        // SchedulerScript CRUD
        create_scheduler_script,
        list_scheduler_scripts,
        get_scheduler_script,
        update_scheduler_script_endpoint,
        delete_scheduler_script_endpoint,
        
        // SchedulerOverride CRUD
        create_scheduler_override,
        list_scheduler_overrides,
        
        // Navigation properties
        get_site_scheduler_scripts,
        get_site_scheduler_overrides,
        
        // Custom actions
        validate_scheduler_script,
        execute_site_scheduler,
        get_site_state
    ]
}