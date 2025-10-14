//! API endpoints for command management.
//!
//! This module provides HTTP endpoints for CRUD operations on commands.
//! Commands are atomic equipment actions associated with sites.
//!
//! # Authorization Rules
//! - Site administrators can perform CRUD operations on commands within their site
//! - newtown-staff and newtown-admin roles can perform CRUD operations on any command

use rocket::Route;
use rocket::http::Status;
use rocket::response::{self, status};
use rocket::serde::json::Json;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::logged_json::LoggedJson;
use crate::models::{Command, NewCommand};
use crate::orm::DbConn;
use crate::orm::command::{
    delete_command, get_active_commands_by_site, get_command_by_id, get_command_by_site_and_name,
    get_commands_by_site, insert_command, update_command,
};
use crate::orm::site::get_site_by_id;
use crate::session_guards::AuthenticatedUser;

/// Error response structure for command API failures.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct ErrorResponse {
    pub error: String,
}

/// Request payload for creating a new command
#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct CreateCommandRequest {
    pub site_id: i32,
    pub name: String,
    pub description: Option<String>,
    pub equipment_type: String,
    pub equipment_id: String,
    pub action: String,
    pub parameters: Option<String>,
    pub is_active: bool,
}

/// Request payload for updating a command
#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct UpdateCommandRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub equipment_type: Option<String>,
    pub equipment_id: Option<String>,
    pub action: Option<String>,
    pub parameters: Option<String>,
    pub is_active: Option<bool>,
}

/// Helper function to check if user can perform CRUD operations on a command's site
fn can_crud_command(user: &AuthenticatedUser, site_company_id: i32) -> bool {
    // newtown-admin and newtown-staff can CRUD any command
    if user.has_any_role(&["newtown-admin", "newtown-staff"]) {
        return true;
    }

    // Company admins can CRUD commands in their own company's sites
    if user.has_role("admin") && user.user.company_id == site_company_id {
        return true;
    }

    false
}

/// Create Command endpoint.
#[post("/1/Commands", data = "<new_command>")]
pub async fn create_command(
    db: DbConn,
    new_command: LoggedJson<CreateCommandRequest>,
    auth_user: AuthenticatedUser,
) -> Result<status::Created<Json<Command>>, response::status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        // Validate that the site exists and check authorization
        match get_site_by_id(conn, new_command.site_id) {
            Ok(Some(site)) => {
                if !can_crud_command(&auth_user, site.company_id) {
                    let err = Json(ErrorResponse {
                        error: "Forbidden: insufficient permissions to create command for this site"
                            .to_string(),
                    });
                    return Err(response::status::Custom(Status::Forbidden, err));
                }

                // Check if command with this name already exists at the site
                match get_command_by_site_and_name(conn, new_command.site_id, &new_command.name) {
                    Ok(Some(_)) => {
                        let err = Json(ErrorResponse {
                            error: format!(
                                "Command with name '{}' already exists at this site",
                                new_command.name
                            ),
                        });
                        return Err(response::status::Custom(Status::Conflict, err));
                    }
                    Ok(None) => {
                        // Proceed with command creation
                        let command_input = NewCommand {
                            site_id: new_command.site_id,
                            name: new_command.name.clone(),
                            description: new_command.description.clone(),
                            equipment_type: new_command.equipment_type.clone(),
                            equipment_id: new_command.equipment_id.clone(),
                            action: new_command.action.clone(),
                            parameters: new_command.parameters.clone(),
                            is_active: new_command.is_active,
                        };

                        insert_command(conn, command_input, Some(auth_user.user.id))
                            .map(|command| status::Created::new("/").body(Json(command)))
                            .map_err(|e| {
                                eprintln!("Error creating command: {:?}", e);
                                let err = Json(ErrorResponse {
                                    error: "Internal server error while creating command".to_string(),
                                });
                                response::status::Custom(Status::InternalServerError, err)
                            })
                    }
                    Err(e) => {
                        eprintln!("Error checking for existing command: {:?}", e);
                        let err = Json(ErrorResponse {
                            error: "Database error while checking for existing command".to_string(),
                        });
                        Err(response::status::Custom(Status::InternalServerError, err))
                    }
                }
            }
            Ok(None) => {
                let err = Json(ErrorResponse {
                    error: format!("Site with ID {} does not exist", new_command.site_id),
                });
                Err(response::status::Custom(Status::BadRequest, err))
            }
            Err(e) => {
                eprintln!("Error validating site for command creation: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Internal server error while validating site".to_string(),
                });
                Err(response::status::Custom(Status::InternalServerError, err))
            }
        }
    })
    .await
}

/// Get Command endpoint.
#[get("/1/Commands/<command_id>")]
pub async fn get_command(
    db: DbConn,
    command_id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Json<Command>, Status> {
    db.run(move |conn| {
        match get_command_by_id(conn, command_id) {
            Ok(Some(command)) => {
                // Get the site to check authorization
                match get_site_by_id(conn, command.site_id) {
                    Ok(Some(site)) => {
                        if !can_crud_command(&auth_user, site.company_id) {
                            return Err(Status::Forbidden);
                        }
                        Ok(Json(command))
                    }
                    Ok(None) => Err(Status::NotFound),
                    Err(e) => {
                        eprintln!("Error getting site for command: {:?}", e);
                        Err(Status::InternalServerError)
                    }
                }
            }
            Ok(None) => Err(Status::NotFound),
            Err(e) => {
                eprintln!("Error getting command: {:?}", e);
                Err(Status::InternalServerError)
            }
        }
    })
    .await
}

/// List Commands by Site endpoint.
#[get("/1/Sites/<site_id>/Commands?<active>")]
pub async fn list_commands_by_site(
    db: DbConn,
    site_id: i32,
    active: Option<bool>,
    auth_user: AuthenticatedUser,
) -> Result<Json<serde_json::Value>, Status> {
    db.run(move |conn| {
        // Check if site exists and validate authorization
        match get_site_by_id(conn, site_id) {
            Ok(Some(site)) => {
                if !can_crud_command(&auth_user, site.company_id) {
                    return Err(Status::Forbidden);
                }

                let commands = if active == Some(true) {
                    get_active_commands_by_site(conn, site_id)
                } else {
                    get_commands_by_site(conn, site_id)
                };

                match commands {
                    Ok(commands) => {
                        let response = serde_json::json!({
                            "@odata.context": format!("http://localhost/api/1/$metadata#Commands"),
                            "value": commands
                        });
                        Ok(Json(response))
                    }
                    Err(e) => {
                        eprintln!("Error listing commands: {:?}", e);
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

/// Update Command endpoint.
#[put("/1/Commands/<command_id>", data = "<update_data>")]
pub async fn update_command_endpoint(
    db: DbConn,
    command_id: i32,
    update_data: LoggedJson<UpdateCommandRequest>,
    auth_user: AuthenticatedUser,
) -> Result<Json<Command>, response::status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        match get_command_by_id(conn, command_id) {
            Ok(Some(command)) => {
                // Check authorization
                match get_site_by_id(conn, command.site_id) {
                    Ok(Some(site)) => {
                        if !can_crud_command(&auth_user, site.company_id) {
                            let err = Json(ErrorResponse {
                                error: "Forbidden: insufficient permissions to update this command"
                                    .to_string(),
                            });
                            return Err(response::status::Custom(Status::Forbidden, err));
                        }

                        // Perform the update
                        update_command(
                            conn,
                            command_id,
                            update_data.name.clone(),
                            update_data.description.clone().map(Some),
                            update_data.equipment_type.clone(),
                            update_data.equipment_id.clone(),
                            update_data.action.clone(),
                            update_data.parameters.clone().map(Some),
                            update_data.is_active,
                            Some(auth_user.user.id),
                        )
                        .map(Json)
                        .map_err(|e| {
                            eprintln!("Error updating command: {:?}", e);
                            let err = Json(ErrorResponse {
                                error: "Internal server error while updating command".to_string(),
                            });
                            response::status::Custom(Status::InternalServerError, err)
                        })
                    }
                    Ok(None) => {
                        let err = Json(ErrorResponse {
                            error: "Site not found for command".to_string(),
                        });
                        Err(response::status::Custom(Status::NotFound, err))
                    }
                    Err(e) => {
                        eprintln!("Error finding site for command: {:?}", e);
                        let err = Json(ErrorResponse {
                            error: "Internal server error while finding site".to_string(),
                        });
                        Err(response::status::Custom(Status::InternalServerError, err))
                    }
                }
            }
            Ok(None) => {
                let err = Json(ErrorResponse {
                    error: format!("Command with ID {} not found", command_id),
                });
                Err(response::status::Custom(Status::NotFound, err))
            }
            Err(e) => {
                eprintln!("Error finding command for update: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Internal server error while finding command".to_string(),
                });
                Err(response::status::Custom(Status::InternalServerError, err))
            }
        }
    })
    .await
}

/// Delete Command endpoint.
#[delete("/1/Commands/<command_id>")]
pub async fn delete_command_endpoint(
    db: DbConn,
    command_id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Status, Status> {
    db.run(move |conn| {
        match get_command_by_id(conn, command_id) {
            Ok(Some(command)) => {
                // Check authorization
                match get_site_by_id(conn, command.site_id) {
                    Ok(Some(site)) => {
                        if !can_crud_command(&auth_user, site.company_id) {
                            return Err(Status::Forbidden);
                        }

                        // Perform the deletion
                        match delete_command(conn, command_id, Some(auth_user.user.id)) {
                            Ok(rows_affected) => {
                                if rows_affected > 0 {
                                    Ok(Status::NoContent)
                                } else {
                                    Err(Status::NotFound)
                                }
                            }
                            Err(e) => {
                                eprintln!("Error deleting command: {:?}", e);
                                Err(Status::InternalServerError)
                            }
                        }
                    }
                    Ok(None) => Err(Status::NotFound),
                    Err(e) => {
                        eprintln!("Error finding site for command: {:?}", e);
                        Err(Status::InternalServerError)
                    }
                }
            }
            Ok(None) => Err(Status::NotFound),
            Err(e) => {
                eprintln!("Error finding command for deletion: {:?}", e);
                Err(Status::InternalServerError)
            }
        }
    })
    .await
}

/// Returns a vector of all routes defined in this module.
pub fn routes() -> Vec<Route> {
    routes![
        create_command,
        get_command,
        list_commands_by_site,
        update_command_endpoint,
        delete_command_endpoint
    ]
}
