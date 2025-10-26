//! API endpoints for device management.
//!
//! This module provides HTTP endpoints for CRUD operations on devices.
//! Devices represent physical equipment located at sites within companies.
//!
//! # Authorization Rules
//! - Company admins can perform CRUD operations on devices within their company
//! - newtown-staff and newtown-admin roles can perform CRUD operations on any
//!   device
//! - Regular users (staff) can view devices in their company but cannot modify
//!   them

use rocket::{Route, http::Status, response::status, serde::json::Json};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{
    models::{Device, DeviceInput},
    odata_query::ODataQuery,
    orm::{
        DbConn,
        device::{
            delete_device, get_all_devices, get_device_by_id, get_device_by_site_and_name,
            get_devices_by_company, insert_device, update_device,
        },
        site::get_site_by_id,
    },
    session_guards::AuthenticatedUser,
};

/// Error response structure for device API failures.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct ErrorResponse {
    pub error: String,
}

/// Request payload for creating a new device
#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct CreateDeviceRequest {
    pub name: Option<String>, // Optional, defaults to type_ if not provided
    pub description: Option<String>,
    #[ts(type = "string")]
    pub type_: String,
    pub model: String,
    pub serial: Option<String>,
    pub ip_address: Option<String>,
    #[ts(type = "string | null")]
    pub install_date: Option<chrono::NaiveDateTime>,
    pub company_id: i32,
    pub site_id: i32,
}

/// Request payload for updating a device (all fields optional except ID
/// constraints)
#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct UpdateDeviceRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    #[ts(type = "string")]
    pub type_: Option<String>,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub ip_address: Option<String>,
    #[ts(type = "string | null")]
    pub install_date: Option<chrono::NaiveDateTime>,
    pub company_id: Option<i32>,
    pub site_id: Option<i32>,
}

/// Helper function to check if user can view devices for a company
fn can_view_devices(user: &AuthenticatedUser, company_id: i32) -> bool {
    // newtown-admin and newtown-staff can view devices in any company
    if user.has_any_role(&["newtown-admin", "newtown-staff"]) {
        return true;
    }

    // Users can view devices in their own company
    if user.user.company_id == company_id {
        return true;
    }

    false
}

/// Helper function to check if user can perform CRUD operations on a device
fn can_crud_device(user: &AuthenticatedUser, device_company_id: i32) -> bool {
    // newtown-admin and newtown-staff can CRUD any device
    if user.has_any_role(&["newtown-admin", "newtown-staff"]) {
        return true;
    }

    // Company admins can CRUD devices in their own company
    if user.has_role("admin") && user.user.company_id == device_company_id {
        return true;
    }

    false
}

/// Create Device endpoint.
///
/// - **URL:** `/api/1/Devices`
/// - **Method:** `POST`
/// - **Purpose:** Creates a new device
/// - **Authentication:** Required
/// - **Authorization:** Company admin (for own company) or
///   newtown-admin/newtown-staff (for any company)
///
/// # Request Format
///
/// ```json
/// {
///   "name": "Main Inverter",
///   "description": "Primary solar inverter",
///   "type_": "Inverter",
///   "model": "SUN2000-100KTL",
///   "serial": "INV20240001",
///   "ip_address": "192.168.1.100",
///   "install_date": "2024-03-15T10:00:00",
///   "company_id": 1,
///   "site_id": 5
/// }
/// ```
///
/// # Response
///
/// **Success (HTTP 201 Created):**
/// Returns the created device with assigned ID
///
/// **Error Responses:**
/// - **401 Unauthorized**: User not authenticated
/// - **403 Forbidden**: User lacks permission to create devices in this company
/// - **404 Not Found**: Site not found or site doesn't belong to company
/// - **400 Bad Request**: Device name already exists at site or validation
///   error
#[post("/1/Devices", data = "<request>")]
pub async fn create_device(
    db: DbConn,
    auth_user: AuthenticatedUser,
    request: Json<CreateDeviceRequest>,
) -> Result<status::Created<Json<Device>>, status::Custom<Json<ErrorResponse>>> {
    let request = request.into_inner();

    db.run(move |conn| {
        // Check if user can create devices for this company
        if !can_crud_device(&auth_user, request.company_id) {
            return Err(status::Custom(
                Status::Forbidden,
                Json(ErrorResponse {
                    error: "Insufficient permissions to create devices in this company".to_string(),
                }),
            ));
        }

        // Verify site exists and belongs to the specified company
        let site = match get_site_by_id(conn, request.site_id) {
            Ok(Some(site)) => site,
            Ok(None) => {
                return Err(status::Custom(
                    Status::NotFound,
                    Json(ErrorResponse { error: "Site not found".to_string() }),
                ));
            }
            Err(_) => {
                return Err(status::Custom(
                    Status::InternalServerError,
                    Json(ErrorResponse {
                        error: "Database error while fetching site".to_string(),
                    }),
                ));
            }
        };

        if site.company_id != request.company_id {
            return Err(status::Custom(
                Status::BadRequest,
                Json(ErrorResponse {
                    error: "Site does not belong to the specified company".to_string(),
                }),
            ));
        }

        // Check if device name already exists at this site (if name provided)
        let device_name = request.name.clone().unwrap_or_else(|| request.type_.clone());
        if let Ok(Some(_)) = get_device_by_site_and_name(conn, request.site_id, &device_name) {
            return Err(status::Custom(
                Status::BadRequest,
                Json(ErrorResponse {
                    error: "Device with this name already exists at this site".to_string(),
                }),
            ));
        }

        let device_input = DeviceInput {
            name: request.name,
            description: request.description,
            type_: request.type_,
            model: request.model,
            serial: request.serial,
            ip_address: request.ip_address,
            install_date: request.install_date,
            company_id: request.company_id,
            site_id: request.site_id,
        };

        match insert_device(conn, device_input, Some(auth_user.user.id)) {
            Ok(device) => {
                let uri = format!("/api/1/Devices/{}", device.id);
                Ok(status::Created::new(uri).body(Json(device)))
            }
            Err(_) => Err(status::Custom(
                Status::InternalServerError,
                Json(ErrorResponse {
                    error: "Failed to create device".to_string(),
                }),
            )),
        }
    })
    .await
}

/// List Devices endpoint.
///
/// - **URL:** `/api/1/Devices`
/// - **Method:** `GET`
/// - **Purpose:** Lists devices with OData query support
/// - **Authentication:** Required
/// - **Authorization:** Users can view devices in their company; newtown roles
///   can view all
///
/// # OData Query Support
/// - `$select`: Choose specific fields
/// - `$filter`: Filter devices by type, site, company, etc.
/// - `$orderby`: Sort devices
/// - `$top/$skip`: Pagination
/// - `$count`: Include total count
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// {
///   "@odata.context": "http://localhost/api/1/$metadata#Devices",
///   "@odata.count": 25,
///   "value": [
///     {
///       "id": 1,
///       "name": "Main Inverter",
///       "type_": "Inverter",
///       "model": "SUN2000-100KTL",
///       "company_id": 1,
///       "site_id": 5
///     }
///   ]
/// }
/// ```
#[get("/1/Devices?<_query..>")]
pub async fn list_devices(
    db: DbConn,
    auth_user: AuthenticatedUser,
    _query: ODataQuery,
) -> Result<Json<serde_json::Value>, Status> {
    db.run(move |conn| {
        let devices = if auth_user.has_any_role(&["newtown-admin", "newtown-staff"]) {
            // Newtown roles can see all devices
            match get_all_devices(conn) {
                Ok(devices) => devices,
                Err(_) => return Err(Status::InternalServerError),
            }
        } else {
            // Regular users can only see devices in their company
            match get_devices_by_company(conn, auth_user.user.company_id) {
                Ok(devices) => devices,
                Err(_) => return Err(Status::InternalServerError),
            }
        };

        // TODO: Apply OData query options (filtering, sorting, pagination)
        // For now, return all devices without query processing

        let response = serde_json::json!({
            "@odata.context": "http://localhost/api/1/$metadata#Devices",
            "value": devices
        });

        Ok(Json(response))
    })
    .await
}

/// Get Device endpoint.
///
/// - **URL:** `/api/1/Devices/{id}`
/// - **Method:** `GET`
/// - **Purpose:** Gets a specific device by ID
/// - **Authentication:** Required
/// - **Authorization:** Users can view devices in their company; newtown roles
///   can view all
#[get("/1/Devices/<device_id>")]
pub async fn get_device(
    db: DbConn,
    auth_user: AuthenticatedUser,
    device_id: i32,
) -> Result<Json<Device>, Status> {
    db.run(move |conn| {
        let device = match get_device_by_id(conn, device_id) {
            Ok(Some(device)) => device,
            Ok(None) => return Err(Status::NotFound),
            Err(_) => return Err(Status::InternalServerError),
        };

        // Check if user can view this device
        if !can_view_devices(&auth_user, device.company_id) {
            return Err(Status::Forbidden);
        }

        Ok(Json(device))
    })
    .await
}

/// Update Device endpoint.
///
/// - **URL:** `/api/1/Devices/{id}`
/// - **Method:** `PUT`
/// - **Purpose:** Updates a device
/// - **Authentication:** Required
/// - **Authorization:** Company admin (for own company) or
///   newtown-admin/newtown-staff (for any company)
#[put("/1/Devices/<device_id>", data = "<request>")]
pub async fn update_device_endpoint(
    db: DbConn,
    auth_user: AuthenticatedUser,
    device_id: i32,
    request: Json<UpdateDeviceRequest>,
) -> Result<Json<Device>, status::Custom<Json<ErrorResponse>>> {
    let request = request.into_inner();

    db.run(move |conn| {
        // Get current device to check permissions
        let current_device = match get_device_by_id(conn, device_id) {
            Ok(Some(device)) => device,
            Ok(None) => {
                return Err(status::Custom(
                    Status::NotFound,
                    Json(ErrorResponse { error: "Device not found".to_string() }),
                ));
            }
            Err(_) => {
                return Err(status::Custom(
                    Status::InternalServerError,
                    Json(ErrorResponse {
                        error: "Database error while fetching device".to_string(),
                    }),
                ));
            }
        };

        // Check if user can modify this device
        if !can_crud_device(&auth_user, current_device.company_id) {
            return Err(status::Custom(
                Status::Forbidden,
                Json(ErrorResponse {
                    error: "Insufficient permissions to update this device".to_string(),
                }),
            ));
        }

        // If changing company/site, validate the new values
        if let (Some(new_company_id), Some(new_site_id)) = (request.company_id, request.site_id) {
            // Verify site exists and belongs to the specified company
            let site = match get_site_by_id(conn, new_site_id) {
                Ok(Some(site)) => site,
                Ok(None) => {
                    return Err(status::Custom(
                        Status::NotFound,
                        Json(ErrorResponse { error: "Site not found".to_string() }),
                    ));
                }
                Err(_) => {
                    return Err(status::Custom(
                        Status::InternalServerError,
                        Json(ErrorResponse {
                            error: "Database error while fetching site".to_string(),
                        }),
                    ));
                }
            };

            if site.company_id != new_company_id {
                return Err(status::Custom(
                    Status::BadRequest,
                    Json(ErrorResponse {
                        error: "Site does not belong to the specified company".to_string(),
                    }),
                ));
            }

            // Check if user can create devices for the new company
            if !can_crud_device(&auth_user, new_company_id) {
                return Err(status::Custom(
                    Status::Forbidden,
                    Json(ErrorResponse {
                        error: "Insufficient permissions to move device to this company"
                            .to_string(),
                    }),
                ));
            }
        }

        // Check for name conflicts if name is being changed
        if let Some(ref new_name) = request.name {
            let target_site_id = request.site_id.unwrap_or(current_device.site_id);
            if let Ok(Some(existing_device)) =
                get_device_by_site_and_name(conn, target_site_id, new_name)
            {
                if existing_device.id != device_id {
                    return Err(status::Custom(
                        Status::BadRequest,
                        Json(ErrorResponse {
                            error: "Device with this name already exists at the target site"
                                .to_string(),
                        }),
                    ));
                }
            }
        }

        match update_device(
            conn,
            device_id,
            request.name,
            request.description.map(Some), // Convert Option<String> to Option<Option<String>>
            request.type_,
            request.model,
            request.serial.map(Some),
            request.ip_address.map(Some),
            request.install_date.map(Some),
            request.company_id,
            request.site_id,
            Some(auth_user.user.id),
        ) {
            Ok(device) => Ok(Json(device)),
            Err(_) => Err(status::Custom(
                Status::InternalServerError,
                Json(ErrorResponse {
                    error: "Failed to update device".to_string(),
                }),
            )),
        }
    })
    .await
}

/// Delete Device endpoint.
///
/// - **URL:** `/api/1/Devices/{id}`
/// - **Method:** `DELETE`
/// - **Purpose:** Deletes a device
/// - **Authentication:** Required
/// - **Authorization:** Company admin (for own company) or
///   newtown-admin/newtown-staff (for any company)
#[delete("/1/Devices/<device_id>")]
pub async fn delete_device_endpoint(
    db: DbConn,
    auth_user: AuthenticatedUser,
    device_id: i32,
) -> Result<Status, status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        // Get current device to check permissions
        let current_device = match get_device_by_id(conn, device_id) {
            Ok(Some(device)) => device,
            Ok(None) => {
                return Err(status::Custom(
                    Status::NotFound,
                    Json(ErrorResponse { error: "Device not found".to_string() }),
                ));
            }
            Err(_) => {
                return Err(status::Custom(
                    Status::InternalServerError,
                    Json(ErrorResponse {
                        error: "Database error while fetching device".to_string(),
                    }),
                ));
            }
        };

        // Check if user can delete this device
        if !can_crud_device(&auth_user, current_device.company_id) {
            return Err(status::Custom(
                Status::Forbidden,
                Json(ErrorResponse {
                    error: "Insufficient permissions to delete this device".to_string(),
                }),
            ));
        }

        match delete_device(conn, device_id, Some(auth_user.user.id)) {
            Ok(_) => Ok(Status::NoContent),
            Err(_) => Err(status::Custom(
                Status::InternalServerError,
                Json(ErrorResponse {
                    error: "Failed to delete device".to_string(),
                }),
            )),
        }
    })
    .await
}

/// Navigation: Get Site for Device endpoint.
///
/// - **URL:** `/api/1/Devices/{id}/Site`
/// - **Method:** `GET`
/// - **Purpose:** Gets the site where a device is located
/// - **Authentication:** Required
/// - **Authorization:** Users can view devices in their company; newtown roles
///   can view all
#[get("/1/Devices/<device_id>/Site")]
pub async fn get_device_site(
    db: DbConn,
    auth_user: AuthenticatedUser,
    device_id: i32,
) -> Result<Json<crate::models::Site>, Status> {
    db.run(move |conn| {
        let device = match get_device_by_id(conn, device_id) {
            Ok(Some(device)) => device,
            Ok(None) => return Err(Status::NotFound),
            Err(_) => return Err(Status::InternalServerError),
        };

        // Check if user can view this device
        if !can_view_devices(&auth_user, device.company_id) {
            return Err(Status::Forbidden);
        }

        let site = match get_site_by_id(conn, device.site_id) {
            Ok(Some(site)) => site,
            Ok(None) => return Err(Status::NotFound),
            Err(_) => return Err(Status::InternalServerError),
        };

        Ok(Json(site))
    })
    .await
}

/// Returns a vector of all routes defined in this module.
pub fn routes() -> Vec<Route> {
    routes![
        create_device,
        list_devices,
        get_device,
        update_device_endpoint,
        delete_device_endpoint,
        get_device_site
    ]
}
