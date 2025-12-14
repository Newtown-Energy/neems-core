//! API endpoints for managing schedule library items.

use rocket::{Route, http::Status, response::status, serde::json::Json};
use serde::Serialize;
use ts_rs::TS;

use crate::{
    logged_json::LoggedJson,
    models::{
        CloneLibraryItemRequest, CreateLibraryItemRequest, ScheduleLibraryItem,
        UpdateLibraryItemRequest,
    },
    orm::{
        DbConn,
        schedule_library::{
            clone_library_item, create_library_item, delete_library_item, get_library_item,
            get_library_items_for_site, update_library_item,
        },
        site::get_site_by_id,
    },
    session_guards::AuthenticatedUser,
};

#[derive(Serialize, TS)]
#[ts(export)]
pub struct ErrorResponse {
    pub error: String,
}

// Helper function to check if user can manage schedules for a site
fn can_manage_schedule(
    user: &AuthenticatedUser,
    site_id: i32,
    conn: &mut diesel::SqliteConnection,
) -> bool {
    // newtown-admin and newtown-staff can manage any schedule
    if user.has_any_role(&["newtown-admin", "newtown-staff"]) {
        return true;
    }

    // Company admins can manage schedules for their company's sites
    if user.has_role("admin") {
        if let Ok(Some(site_data)) = get_site_by_id(conn, site_id) {
            return site_data.company_id == user.user.company_id;
        }
    }

    false
}

// Helper function to check if user can view schedules for a site
fn can_view_schedule(
    user: &AuthenticatedUser,
    site_id: i32,
    conn: &mut diesel::SqliteConnection,
) -> bool {
    // newtown-admin and newtown-staff can view any schedule
    if user.has_any_role(&["newtown-admin", "newtown-staff"]) {
        return true;
    }

    // Any user can view schedules for their company's sites
    if let Ok(Some(site_data)) = get_site_by_id(conn, site_id) {
        return site_data.company_id == user.user.company_id;
    }

    false
}

/// List library items for a site
#[get("/1/Sites/<site_id>/ScheduleLibraryItems")]
pub async fn list_library_items(
    db: DbConn,
    site_id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Json<Vec<ScheduleLibraryItem>>, status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        // Check authorization
        if !can_view_schedule(&auth_user, site_id, conn) {
            let err = Json(ErrorResponse {
                error: "Forbidden: insufficient permissions".to_string(),
            });
            return Err(status::Custom(Status::Forbidden, err));
        }

        // Ensure default schedule exists
        use crate::orm::schedule_library::ensure_default_schedule_exists;
        let _ = ensure_default_schedule_exists(conn, site_id, Some(auth_user.user.id));

        get_library_items_for_site(conn, site_id).map(Json).map_err(|e| {
            eprintln!("Error listing library items: {:?}", e);
            let err = Json(ErrorResponse {
                error: "Internal server error".to_string(),
            });
            status::Custom(Status::InternalServerError, err)
        })
    })
    .await
}

/// Get a single library item by ID
#[get("/1/ScheduleLibraryItems/<id>")]
pub async fn get_library_item_by_id(
    db: DbConn,
    id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Json<ScheduleLibraryItem>, status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        match get_library_item(conn, id) {
            Ok(item) => {
                // Check authorization
                if !can_view_schedule(&auth_user, item.site_id, conn) {
                    let err = Json(ErrorResponse {
                        error: "Forbidden: insufficient permissions".to_string(),
                    });
                    return Err(status::Custom(Status::Forbidden, err));
                }
                Ok(Json(item))
            }
            Err(diesel::result::Error::NotFound) => {
                let err = Json(ErrorResponse {
                    error: "Library item not found".to_string(),
                });
                Err(status::Custom(Status::NotFound, err))
            }
            Err(e) => {
                eprintln!("Error getting library item: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Internal server error".to_string(),
                });
                Err(status::Custom(Status::InternalServerError, err))
            }
        }
    })
    .await
}

/// Create a new library item
#[post("/1/Sites/<site_id>/ScheduleLibraryItems", data = "<request>")]
pub async fn create_library_item_endpoint(
    db: DbConn,
    site_id: i32,
    request: LoggedJson<CreateLibraryItemRequest>,
    auth_user: AuthenticatedUser,
) -> Result<status::Created<Json<ScheduleLibraryItem>>, status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        // Check authorization
        if !can_manage_schedule(&auth_user, site_id, conn) {
            let err = Json(ErrorResponse {
                error: "Forbidden: insufficient permissions".to_string(),
            });
            return Err(status::Custom(Status::Forbidden, err));
        }

        match create_library_item(conn, site_id, request.into_inner(), Some(auth_user.user.id)) {
            Ok(item) => {
                let location = format!("/api/1/ScheduleLibraryItems/{}", item.id);
                Ok(status::Created::new(location).body(Json(item)))
            }
            Err(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            )) => {
                let err = Json(ErrorResponse {
                    error: "A schedule with this name already exists".to_string(),
                });
                Err(status::Custom(Status::BadRequest, err))
            }
            Err(e) => {
                eprintln!("Error creating library item: {:?}", e);
                let err = Json(ErrorResponse {
                    error: format!("Error creating library item: {}", e),
                });
                Err(status::Custom(Status::InternalServerError, err))
            }
        }
    })
    .await
}

/// Update a library item
#[put("/1/ScheduleLibraryItems/<id>", data = "<request>")]
pub async fn update_library_item_endpoint(
    db: DbConn,
    id: i32,
    request: LoggedJson<UpdateLibraryItemRequest>,
    auth_user: AuthenticatedUser,
) -> Result<Json<ScheduleLibraryItem>, status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        // First get the item to check site_id
        let existing = match get_library_item(conn, id) {
            Ok(item) => item,
            Err(diesel::result::Error::NotFound) => {
                let err = Json(ErrorResponse {
                    error: "Library item not found".to_string(),
                });
                return Err(status::Custom(Status::NotFound, err));
            }
            Err(e) => {
                eprintln!("Error getting library item: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Internal server error".to_string(),
                });
                return Err(status::Custom(Status::InternalServerError, err));
            }
        };

        // Check authorization
        if !can_manage_schedule(&auth_user, existing.site_id, conn) {
            let err = Json(ErrorResponse {
                error: "Forbidden: insufficient permissions".to_string(),
            });
            return Err(status::Custom(Status::Forbidden, err));
        }

        match update_library_item(conn, id, request.into_inner(), Some(auth_user.user.id)) {
            Ok(item) => Ok(Json(item)),
            Err(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            )) => {
                let err = Json(ErrorResponse {
                    error: "A schedule with this name already exists".to_string(),
                });
                Err(status::Custom(Status::BadRequest, err))
            }
            Err(e) => {
                eprintln!("Error updating library item: {:?}", e);
                let err = Json(ErrorResponse {
                    error: format!("Error updating library item: {}", e),
                });
                Err(status::Custom(Status::InternalServerError, err))
            }
        }
    })
    .await
}

/// Delete a library item
#[delete("/1/ScheduleLibraryItems/<id>")]
pub async fn delete_library_item_endpoint(
    db: DbConn,
    id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Status, status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        // First get the item to check site_id
        let existing = match get_library_item(conn, id) {
            Ok(item) => item,
            Err(diesel::result::Error::NotFound) => {
                let err = Json(ErrorResponse {
                    error: "Library item not found".to_string(),
                });
                return Err(status::Custom(Status::NotFound, err));
            }
            Err(e) => {
                eprintln!("Error getting library item: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Internal server error".to_string(),
                });
                return Err(status::Custom(Status::InternalServerError, err));
            }
        };

        // Check authorization
        if !can_manage_schedule(&auth_user, existing.site_id, conn) {
            let err = Json(ErrorResponse {
                error: "Forbidden: insufficient permissions".to_string(),
            });
            return Err(status::Custom(Status::Forbidden, err));
        }

        match delete_library_item(conn, id, Some(auth_user.user.id)) {
            Ok(_) => Ok(Status::NoContent),
            Err(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::ForeignKeyViolation,
                _,
            )) => {
                let err = Json(ErrorResponse {
                    error: "Cannot delete the default schedule".to_string(),
                });
                Err(status::Custom(Status::BadRequest, err))
            }
            Err(e) => {
                eprintln!("Error deleting library item: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Internal server error".to_string(),
                });
                Err(status::Custom(Status::InternalServerError, err))
            }
        }
    })
    .await
}

/// Clone a library item
#[post("/1/ScheduleLibraryItems/<id>/Clone", data = "<request>")]
pub async fn clone_library_item_endpoint(
    db: DbConn,
    id: i32,
    request: LoggedJson<CloneLibraryItemRequest>,
    auth_user: AuthenticatedUser,
) -> Result<status::Created<Json<ScheduleLibraryItem>>, status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        // First get the item to check site_id
        let existing = match get_library_item(conn, id) {
            Ok(item) => item,
            Err(diesel::result::Error::NotFound) => {
                let err = Json(ErrorResponse {
                    error: "Library item not found".to_string(),
                });
                return Err(status::Custom(Status::NotFound, err));
            }
            Err(e) => {
                eprintln!("Error getting library item: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Internal server error".to_string(),
                });
                return Err(status::Custom(Status::InternalServerError, err));
            }
        };

        // Check authorization
        if !can_manage_schedule(&auth_user, existing.site_id, conn) {
            let err = Json(ErrorResponse {
                error: "Forbidden: insufficient permissions".to_string(),
            });
            return Err(status::Custom(Status::Forbidden, err));
        }

        let req = request.into_inner();
        match clone_library_item(conn, id, req.name, req.description, Some(auth_user.user.id)) {
            Ok(item) => {
                let location = format!("/api/1/ScheduleLibraryItems/{}", item.id);
                Ok(status::Created::new(location).body(Json(item)))
            }
            Err(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            )) => {
                let err = Json(ErrorResponse {
                    error: "A schedule with this name already exists".to_string(),
                });
                Err(status::Custom(Status::BadRequest, err))
            }
            Err(e) => {
                eprintln!("Error cloning library item: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Internal server error".to_string(),
                });
                Err(status::Custom(Status::InternalServerError, err))
            }
        }
    })
    .await
}

pub fn routes() -> Vec<Route> {
    routes![
        list_library_items,
        get_library_item_by_id,
        create_library_item_endpoint,
        update_library_item_endpoint,
        delete_library_item_endpoint,
        clone_library_item_endpoint,
    ]
}
