//! API endpoints for schedule_command management.
//!
//! This module provides HTTP endpoints for CRUD operations on
//! schedule_commands. ScheduleCommands are atomic equipment actions associated
//! with sites.
//!
//! # Authorization Rules
//! - Site administrators can perform CRUD operations on schedule_commands
//!   within their site
//! - newtown-staff and newtown-admin roles can perform CRUD operations on any
//!   schedule_command

use rocket::{
    Route,
    http::Status,
    response::{self, status},
    serde::json::Json,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    logged_json::LoggedJson,
    models::{NewScheduleCommand, ScheduleCommand},
    orm::{
        DbConn,
        schedule_command::{
            delete_schedule_command, get_active_schedule_commands_by_site,
            get_schedule_command_by_id, get_schedule_commands_by_site, insert_schedule_command,
            update_schedule_command,
        },
        site::get_site_by_id,
    },
    session_guards::AuthenticatedUser,
};

/// Error response structure for schedule_command API failures.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct ErrorResponse {
    pub error: String,
}

/// Request payload for creating a new schedule_command
#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct CreateScheduleCommandRequest {
    pub site_id: i32,
    #[serde(rename = "type")]
    pub type_: crate::models::CommandType,
    pub parameters: Option<String>,
    pub is_active: bool,
}

/// Request payload for updating a schedule_command
#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct UpdateScheduleCommandRequest {
    #[serde(rename = "type")]
    pub type_: Option<crate::models::CommandType>,
    pub parameters: Option<String>,
    pub is_active: Option<bool>,
}

/// Helper function to check if user can perform CRUD operations on a
/// schedule_command's site
fn can_crud_schedule_command(user: &AuthenticatedUser, site_company_id: i32) -> bool {
    // newtown-admin and newtown-staff can CRUD any schedule_command
    if user.has_any_role(&["newtown-admin", "newtown-staff"]) {
        return true;
    }

    // Company admins can CRUD schedule_commands in their own company's sites
    if user.has_role("admin") && user.user.company_id == site_company_id {
        return true;
    }

    false
}

/// Create ScheduleCommand endpoint.
#[post("/1/ScheduleCommands", data = "<new_schedule_command>")]
pub async fn create_schedule_command(
    db: DbConn,
    new_schedule_command: LoggedJson<CreateScheduleCommandRequest>,
    auth_user: AuthenticatedUser,
) -> Result<status::Created<Json<ScheduleCommand>>, response::status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        // Validate that the site exists and check authorization
        match get_site_by_id(conn, new_schedule_command.site_id) {
            Ok(Some(site)) => {
                if !can_crud_schedule_command(&auth_user, site.company_id) {
                    let err = Json(ErrorResponse {
                        error:
                            "Forbidden: insufficient permissions to create schedule_command for this site"
                                .to_string(),
                    });
                    return Err(response::status::Custom(Status::Forbidden, err));
                }

                // Proceed with schedule_command creation
                let schedule_command_input = NewScheduleCommand {
                    site_id: new_schedule_command.site_id,
                    type_: new_schedule_command.type_,
                    parameters: new_schedule_command.parameters.clone(),
                    is_active: new_schedule_command.is_active,
                };

                insert_schedule_command(conn, schedule_command_input, Some(auth_user.user.id))
                    .map(|schedule_command| status::Created::new("/").body(Json(schedule_command)))
                    .map_err(|e| {
                        eprintln!("Error creating schedule_command: {:?}", e);
                        let err = Json(ErrorResponse {
                            error: "Internal server error while creating schedule_command"
                                .to_string(),
                        });
                        response::status::Custom(Status::InternalServerError, err)
                    })
            }
            Ok(None) => {
                let err = Json(ErrorResponse {
                    error: format!("Site with ID {} does not exist", new_schedule_command.site_id),
                });
                Err(response::status::Custom(Status::BadRequest, err))
            }
            Err(e) => {
                eprintln!("Error validating site for schedule_command creation: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Internal server error while validating site".to_string(),
                });
                Err(response::status::Custom(Status::InternalServerError, err))
            }
        }
    })
    .await
}

/// Get ScheduleCommand endpoint.
#[get("/1/ScheduleCommands/<schedule_command_id>")]
pub async fn get_schedule_command(
    db: DbConn,
    schedule_command_id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Json<ScheduleCommand>, Status> {
    db.run(move |conn| {
        match get_schedule_command_by_id(conn, schedule_command_id) {
            Ok(Some(schedule_command)) => {
                // Get the site to check authorization
                match get_site_by_id(conn, schedule_command.site_id) {
                    Ok(Some(site)) => {
                        if !can_crud_schedule_command(&auth_user, site.company_id) {
                            return Err(Status::Forbidden);
                        }
                        Ok(Json(schedule_command))
                    }
                    Ok(None) => Err(Status::NotFound),
                    Err(e) => {
                        eprintln!("Error getting site for schedule_command: {:?}", e);
                        Err(Status::InternalServerError)
                    }
                }
            }
            Ok(None) => Err(Status::NotFound),
            Err(e) => {
                eprintln!("Error getting schedule_command: {:?}", e);
                Err(Status::InternalServerError)
            }
        }
    })
    .await
}

/// List ScheduleCommands by Site endpoint.
#[get("/1/Sites/<site_id>/ScheduleCommands?<active>")]
pub async fn list_schedule_commands_by_site(
    db: DbConn,
    site_id: i32,
    active: Option<bool>,
    auth_user: AuthenticatedUser,
) -> Result<Json<serde_json::Value>, Status> {
    db.run(move |conn| {
        // Check if site exists and validate authorization
        match get_site_by_id(conn, site_id) {
            Ok(Some(site)) => {
                if !can_crud_schedule_command(&auth_user, site.company_id) {
                    return Err(Status::Forbidden);
                }

                let schedule_commands = if active == Some(true) {
                    get_active_schedule_commands_by_site(conn, site_id)
                } else {
                    get_schedule_commands_by_site(conn, site_id)
                };

                match schedule_commands {
                    Ok(schedule_commands) => {
                        let response = serde_json::json!({
                            "@odata.context": format!("http://localhost/api/1/$metadata#ScheduleCommands"),
                            "value": schedule_commands
                        });
                        Ok(Json(response))
                    }
                    Err(e) => {
                        eprintln!("Error listing schedule_commands: {:?}", e);
                        Err(Status::InternalServerError)
                    }
                }
            }
            Ok(None) => Err(Status::NotFound),
            Err(e) => {
                eprintln!("Error finding site: {:?}", e);
                Err(Status::InternalServerError)
            }
        }
    })
    .await
}

/// Update ScheduleCommand endpoint.
#[put("/1/ScheduleCommands/<schedule_command_id>", data = "<update_data>")]
pub async fn update_schedule_command_endpoint(
    db: DbConn,
    schedule_command_id: i32,
    update_data: LoggedJson<UpdateScheduleCommandRequest>,
    auth_user: AuthenticatedUser,
) -> Result<Json<ScheduleCommand>, response::status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        match get_schedule_command_by_id(conn, schedule_command_id) {
            Ok(Some(schedule_command)) => {
                // Check authorization
                match get_site_by_id(conn, schedule_command.site_id) {
                    Ok(Some(site)) => {
                        if !can_crud_schedule_command(&auth_user, site.company_id) {
                            let err = Json(ErrorResponse {
                                error: "Forbidden: insufficient permissions to update this schedule_command"
                                    .to_string(),
                            });
                            return Err(response::status::Custom(Status::Forbidden, err));
                        }

                        // Perform the update
                        update_schedule_command(
                            conn,
                            schedule_command_id,
                            update_data.type_,
                            update_data.parameters.clone().map(Some),
                            update_data.is_active,
                            Some(auth_user.user.id),
                        )
                        .map(Json)
                        .map_err(|e| {
                            eprintln!("Error updating schedule_command: {:?}", e);
                            let err = Json(ErrorResponse {
                                error: "Internal server error while updating schedule_command".to_string(),
                            });
                            response::status::Custom(Status::InternalServerError, err)
                        })
                    }
                    Ok(None) => {
                        let err = Json(ErrorResponse {
                            error: "Site not found for schedule_command".to_string(),
                        });
                        Err(response::status::Custom(Status::NotFound, err))
                    }
                    Err(e) => {
                        eprintln!("Error finding site for schedule_command: {:?}", e);
                        let err = Json(ErrorResponse {
                            error: "Internal server error while finding site".to_string(),
                        });
                        Err(response::status::Custom(Status::InternalServerError, err))
                    }
                }
            }
            Ok(None) => {
                let err = Json(ErrorResponse {
                    error: format!("ScheduleCommand with ID {} not found", schedule_command_id),
                });
                Err(response::status::Custom(Status::NotFound, err))
            }
            Err(e) => {
                eprintln!("Error finding schedule_command for update: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Internal server error while finding schedule_command".to_string(),
                });
                Err(response::status::Custom(Status::InternalServerError, err))
            }
        }
    })
    .await
}

/// Delete ScheduleCommand endpoint.
#[delete("/1/ScheduleCommands/<schedule_command_id>")]
pub async fn delete_schedule_command_endpoint(
    db: DbConn,
    schedule_command_id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Status, Status> {
    db.run(move |conn| {
        match get_schedule_command_by_id(conn, schedule_command_id) {
            Ok(Some(schedule_command)) => {
                // Check authorization
                match get_site_by_id(conn, schedule_command.site_id) {
                    Ok(Some(site)) => {
                        if !can_crud_schedule_command(&auth_user, site.company_id) {
                            return Err(Status::Forbidden);
                        }

                        // Perform the deletion
                        match delete_schedule_command(
                            conn,
                            schedule_command_id,
                            Some(auth_user.user.id),
                        ) {
                            Ok(rows_affected) => {
                                if rows_affected > 0 {
                                    Ok(Status::NoContent)
                                } else {
                                    Err(Status::NotFound)
                                }
                            }
                            Err(e) => {
                                eprintln!("Error deleting schedule_command: {:?}", e);
                                Err(Status::InternalServerError)
                            }
                        }
                    }
                    Ok(None) => Err(Status::NotFound),
                    Err(e) => {
                        eprintln!("Error finding site for schedule_command: {:?}", e);
                        Err(Status::InternalServerError)
                    }
                }
            }
            Ok(None) => Err(Status::NotFound),
            Err(e) => {
                eprintln!("Error finding schedule_command for deletion: {:?}", e);
                Err(Status::InternalServerError)
            }
        }
    })
    .await
}

/// Returns a vector of all routes defined in this module.
pub fn routes() -> Vec<Route> {
    routes![
        create_schedule_command,
        get_schedule_command,
        list_schedule_commands_by_site,
        update_schedule_command_endpoint,
        delete_schedule_command_endpoint
    ]
}
