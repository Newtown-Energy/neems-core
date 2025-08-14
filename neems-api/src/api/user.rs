//! API endpoints for managing users.
//!
//! This module provides HTTP endpoints for creating and listing users,
//! along with utility functions for generating test data and helper functions
//! for API testing.

use rand::prelude::IndexedRandom;
use rand::rng;
use rocket::Route;
use rocket::http::{ContentType, Status};
use rocket::local::asynchronous::Client;
use rocket::response::{self, status};
use rocket::serde::Serialize;
use rocket::serde::json::{Json, json};

use crate::logged_json::LoggedJson;
use crate::models::{CompanyInput, Role, UserInput, UserWithRoles};
use crate::odata_query::{ODataQuery, ODataCollectionResponse, build_context_url, apply_select};
use crate::orm::DbConn;
use crate::orm::company::get_company_by_name;
use crate::orm::role::get_role_by_name;
use crate::orm::user::{
    delete_user_with_cleanup, get_user, get_user_by_email, get_user_with_roles,
    get_users_by_company_with_roles, insert_user, list_all_users_with_roles, update_user,
};
use crate::orm::user_role::{assign_user_role_by_name, get_user_roles, remove_user_role_by_name};
use crate::session_guards::AuthenticatedUser;
use ts_rs::TS;

/// Error response structure for user API failures.
#[derive(Serialize, TS)]
#[ts(export)]
pub struct ErrorResponse {
    pub error: String,
}

/// Generates a random selection of usernames for testing purposes.
///
/// This function returns a vector of randomly selected usernames from a
/// predefined list of test usernames. It's primarily used for generating
/// test data and populating development environments.
///
/// # Arguments
/// * `count` - The number of random usernames to select
///
/// # Returns
/// A vector of randomly selected username strings
pub fn random_usernames(count: usize) -> Vec<&'static str> {
    let names = vec![
        "a.johnson",
        "b.williams",
        "c.miller",
        "d.davis",
        "e.rodriguez",
        "f.martinez",
        "g.lee",
        "h.wilson",
        "i.clark",
        "j.hernandez",
        "k.young",
        "l.walker",
        "m.hall",
        "n.allen",
        "o.green",
        "p.adams",
        "q.nelson",
        "r.mitchell",
        "s.carter",
        "t.roberts",
        "amandak",
        "brandonp",
        "chrisl",
        "davidm",
        "ericb",
        "frankr",
        "garys",
        "heathert",
        "ianw",
        "jenniferg",
        "kevinh",
        "lisac",
        "michaeld",
        "nicolef",
        "oliverj",
        "patrickt",
        "quincyv",
        "rachelm",
        "stevenn",
        "taylorq",
        "jameskw1",
        "sarahml2",
        "robertdf3",
        "laurajg4",
        "thomasap5",
        "emilyrs6",
        "danielkt7",
        "megandw8",
        "ryanbh9",
        "oliviamc10",
        "aljohnson",
        "bkmartin",
        "cjwilson",
        "dlthomas",
        "emharris",
        "fnmoore",
        "gpgarcia",
        "hrjackson",
        "iswhite",
        "jdtaylor",
        "browns",
        "moorej",
        "evansm",
        "kingr",
        "wrighta",
        "scottl",
        "riverak",
        "hayesd",
        "collinsp",
        "murphyb",
        "mikescott",
        "aligray",
        "chrismyers",
        "jenngreen",
        "robhall",
        "davecook",
        "sarahkim",
        "timnguyen",
        "katediaz",
        "jimreed",
        "analyst_amy",
        "director_mark",
        "manager_lisa",
        "tech_sam",
        "scientist_raj",
        "ops_carlos",
        "ceo_adam",
        "cto_priya",
        "designer_tom",
        "specialist_lee",
        "wind_mike",
        "nuclear_dave",
        "battery_lucy",
        "grid_omar",
        "fusion_anna",
        "hydro_ryan",
        "solar_priya",
        "storage_paul",
        "transmission_ella",
        "renewables_jack",
        "a.kumar24",
        "b.liang2024",
        "c.patel_eng",
        "d.yang_ops",
        "e.choi_tech",
        "f.singh1",
        "g.wu2023",
        "h.garcia_ce",
        "i.vargas_pe",
        "j.nguyen_lead",
        "alexclark",
        "briancook",
        "carolynlee",
        "davidbrown",
        "ericawang",
        "franklinm",
        "gracehill",
        "henryford",
        "ivyzhang",
        "jasonpark",
        "volts_ryan",
        "amp_anne",
        "watt_dan",
        "joule_mary",
        "ohm_steve",
        "grid_master",
        "solar_expert",
        "wind_tech",
        "nuke_ops",
        "fusion_research",
        "battery_ai",
        "smartgrid_pro",
        "renewables_lead",
        "carbon_zero",
        "green_volt",
        "energy_analyst",
        "power_engineer",
        "grid_designer",
        "sustainability_1",
        "clean_energy_22",
        "ceo_johnson",
        "cfo_smith",
        "cto_lee",
        "vp_operations",
        "director_energy",
        "head_rd",
        "manager_grid",
        "lead_engineer",
        "senior_designer",
        "principal_tech",
        "engineer1",
        "systems_ops",
        "grid_analyst",
        "nuke_specialist",
        "solar_tech",
        "wind_engineer",
        "battery_design",
        "transmission_pro",
        "power_ops",
        "fusion_researcher",
        "hr_jane",
        "finance_mike",
        "legal_lisa",
        "admin_alex",
        "it_support",
        "comms_dan",
        "pr_sarah",
        "facilities_tom",
        "security_lead",
        "logistics_team",
        "jdoe_energy",
        "asmith_power",
        "rlee_solar",
        "kwang_grid",
        "tchen_nuke",
        "lrod_fusion",
        "pmartin_wind",
        "sgarcia_storage",
        "dwilson_ops",
        "ajames_ce",
        "bkim_tech",
        "clopez_eng",
        "dhall_design",
        "eyoung_analyst",
        "fscott_lead",
        "gadams_rd",
        "hbaker_sys",
        "igray_ai",
        "jflores_data",
        "kharris_coo",
        "lmurphy_cfo",
        "mrivera_cto",
        "npham_vp",
        "opark_dir",
        "pcole_mgr",
        "qedwards_hr",
        "rfoster_fin",
        "snguyen_legal",
        "tross_it",
        "upatel_admin",
    ];
    let mut rng = rng();
    let selected: Vec<_> = names.choose_multiple(&mut rng, count).copied().collect();
    selected
}

/// Helper function to create a user via the API and return the created UserWithRoles.
///
/// This function is primarily used for testing purposes. It makes a POST request
/// to the user creation endpoint and returns the newly created user object with roles.
/// It assigns a default "staff" role if none is specified.
///
/// # Arguments
/// * `client` - The Rocket test client instance
/// * `user` - The user data to create (without timestamp fields)
///
/// # Returns
/// The created UserWithRoles object with all fields populated
///
/// # Panics
/// This function will panic if the API request fails or returns invalid data,
/// as it's intended for testing scenarios where such failures indicate test problems.
pub async fn create_user_by_api(client: &Client, user: &UserInput) -> UserWithRoles {
    let body = json!({
        "email": &user.email,
        "password_hash": &user.password_hash,
        "company_id": user.company_id,
        "totp_secret": user.totp_secret,
        "role_names": ["staff"]
    })
    .to_string();
    let response = client
        .post("/api/1/Users")
        .header(ContentType::JSON)
        .body(body)
        .dispatch()
        .await;

    assert_eq!(response.status(), rocket::http::Status::Created);

    response
        .into_json::<UserWithRoles>()
        .await
        .expect("valid UserWithRoles JSON response")
}

/// Helper function to create a user with specific roles via the API.
///
/// This function is primarily used for testing purposes. It makes a POST request
/// to the user creation endpoint with specified roles and returns the newly created user object.
///
/// # Arguments
/// * `client` - The Rocket test client instance
/// * `user` - The user data to create (without timestamp fields)
/// * `role_names` - The roles to assign to the user
///
/// # Returns
/// The created UserWithRoles object with all fields populated
///
/// # Panics
/// This function will panic if the API request fails or returns invalid data,
/// as it's intended for testing scenarios where such failures indicate test problems.
pub async fn create_user_with_roles_by_api(
    client: &Client,
    user: &UserInput,
    role_names: &[&str],
) -> UserWithRoles {
    let body = json!({
        "email": &user.email,
        "password_hash": &user.password_hash,
        "company_id": user.company_id,
        "totp_secret": user.totp_secret,
        "role_names": role_names
    })
    .to_string();
    let response = client
        .post("/api/1/Users")
        .header(ContentType::JSON)
        .body(body)
        .dispatch()
        .await;

    assert_eq!(response.status(), rocket::http::Status::Created);

    response
        .into_json::<UserWithRoles>()
        .await
        .expect("valid UserWithRoles JSON response")
}

/// Create User endpoint.
///
/// - **URL:** `/api/1/users`
/// - **Method:** `POST`
/// - **Purpose:** Creates a new user in the system with assigned roles
/// - **Authentication:** Required
///
/// This endpoint accepts a JSON payload containing user information and
/// role assignments, creates a new user record in the database, and assigns
/// the specified roles in a single operation. At least one role must be provided.
///
/// # Request Format
///
/// ```json
/// {
///   "email": "newuser@example.com",
///   "password_hash": "hashed_password_string",
///   "company_id": 1,
///   "totp_secret": "optional_totp_secret",
///   "role_names": ["admin", "staff"]
/// }
/// ```
///
/// # Response
///
/// **Success (HTTP 201 Created):**
/// ```json
/// {
///   "id": 123,
///   "email": "newuser@example.com",
///   "password_hash": "hashed_password_string",
///   "company_id": 1,
///   "totp_secret": "optional_totp_secret",
///   "created_at": "2023-01-01T00:00:00Z",
///   "updated_at": "2023-01-01T00:00:00Z",
///   "roles": [
///     {
///       "id": 1,
///       "name": "admin",
///       "description": "Administrator role"
///     },
///     {
///       "id": 2,
///       "name": "user",
///       "description": "Basic user role"
///     }
///   ]
/// }
/// ```
///
/// **Failure (HTTP 400 Bad Request):**
/// ```json
/// { "error": "At least one role must be provided" }
/// { "error": "Role 'invalid-role' does not exist" }
/// ```
///
/// **Failure (HTTP 403 Forbidden):**
/// ```json
/// { "error": "Insufficient permissions to create users" }
/// { "error": "Insufficient permissions to assign role 'admin'" }
/// { "error": "Role 'newtown-admin' is restricted to Newtown Energy company" }
/// ```
///
/// **Failure (HTTP 409 Conflict):**
/// ```json
/// { "error": "User with this email already exists" }
/// ```
///
/// **Failure (HTTP 500 Internal Server Error):**
/// ```json
/// { "error": "Database error while creating user" }
/// { "error": "Failed to assign role 'admin' to user" }
/// ```
///
/// # Arguments
/// * `db` - Database connection pool
/// * `new_user` - JSON payload containing the new user data and role assignments
///
/// # Returns
/// * `Ok(status::Created<Json<UserWithRoles>>)` - Successfully created user with roles
/// * `Err(response::status::Custom<Json<ErrorResponse>>)` - Error during creation with JSON error details
#[post("/1/Users", data = "<new_user>")]
pub async fn create_user(
    db: DbConn,
    new_user: LoggedJson<CreateUserWithRolesRequest>,
    auth_user: AuthenticatedUser,
) -> Result<status::Created<Json<UserWithRoles>>, response::status::Custom<Json<ErrorResponse>>> {
    // Check authorization: can create users for target company?
    let target_company_id = new_user.company_id;

    let can_create = if auth_user.has_any_role(&["newtown-admin", "newtown-staff"]) {
        // newtown-admin and newtown-staff can create users for any company
        true
    } else if auth_user.has_role("admin") {
        // admin can only create users for their own company
        auth_user.user.company_id == target_company_id
    } else {
        false
    };

    if !can_create {
        let err = Json(ErrorResponse {
            error: "Insufficient permissions to create users".to_string(),
        });
        return Err(response::status::Custom(Status::Forbidden, err));
    }

    // Validate that at least one role is provided
    if new_user.role_names.is_empty() {
        let err = Json(ErrorResponse {
            error: "At least one role must be provided".to_string(),
        });
        return Err(response::status::Custom(Status::BadRequest, err));
    }

    db.run(move |conn| {
        let user_request = new_user.into_inner();

        // FIRST: Validate all roles exist and user can assign them
        for role_name in &user_request.role_names {
            // Check if role exists
            match get_role_by_name(conn, role_name) {
                Ok(Some(_role)) => {
                    // Role exists, continue with authorization check
                }
                Ok(None) => {
                    let err = Json(ErrorResponse {
                        error: format!("Role '{}' does not exist", role_name),
                    });
                    return Err(response::status::Custom(Status::BadRequest, err));
                }
                Err(e) => {
                    eprintln!("Error checking role existence: {:?}", e);
                    let err = Json(ErrorResponse {
                        error: "Database error while validating roles".to_string(),
                    });
                    return Err(response::status::Custom(Status::InternalServerError, err));
                }
            }

            // Check if user can assign this role (same logic as add_user_role)
            let can_assign = if auth_user.has_role("newtown-admin") {
                // newtown-admin can assign any role
                true
            } else if auth_user.has_role("newtown-staff") {
                // newtown-staff can assign any role except newtown-admin
                role_name != "newtown-admin"
            } else if auth_user.has_role("admin") {
                // admin can assign any role to users in same company
                auth_user.user.company_id == user_request.company_id
            } else {
                false
            };

            if !can_assign {
                let err = Json(ErrorResponse {
                    error: format!("Insufficient permissions to assign role '{}'", role_name),
                });
                return Err(response::status::Custom(Status::Forbidden, err));
            }

            // Check if role is newtown-staff or newtown-admin (company restriction)
            if role_name == "newtown-staff" || role_name == "newtown-admin" {
                let newtown_company_search = CompanyInput {
                    name: "Newtown Energy".to_string(),
                };
                let newtown_company = match get_company_by_name(conn, &newtown_company_search) {
                    Ok(Some(company)) => company,
                    Ok(None) => {
                        eprintln!("Newtown Energy company not found");
                        let err = Json(ErrorResponse {
                            error: "Newtown Energy company not found".to_string(),
                        });
                        return Err(response::status::Custom(Status::InternalServerError, err));
                    }
                    Err(e) => {
                        eprintln!("Error getting Newtown Energy company: {:?}", e);
                        let err = Json(ErrorResponse {
                            error: "Database error while validating company".to_string(),
                        });
                        return Err(response::status::Custom(Status::InternalServerError, err));
                    }
                };

                if user_request.company_id != newtown_company.id {
                    let err = Json(ErrorResponse {
                        error: format!(
                            "Role '{}' is restricted to Newtown Energy company",
                            role_name
                        ),
                    });
                    return Err(response::status::Custom(Status::Forbidden, err));
                }
            }
        }

        // SECOND: Check if user with this email already exists
        match get_user_by_email(conn, &user_request.email) {
            Ok(Some(_existing_user)) => {
                // User with this email already exists
                let err = Json(ErrorResponse {
                    error: "User with this email already exists".to_string(),
                });
                return Err(response::status::Custom(Status::Conflict, err));
            }
            Ok(None) => {
                // User doesn't exist, we can proceed
            }
            Err(e) => {
                eprintln!("Error checking for existing user: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Database error while checking for existing user".to_string(),
                });
                return Err(response::status::Custom(Status::InternalServerError, err));
            }
        }

        // THIRD: Create the user (now that all roles are validated and email is unique)
        let user_no_time = UserInput {
            email: user_request.email,
            password_hash: user_request.password_hash,
            company_id: user_request.company_id,
            totp_secret: user_request.totp_secret,
        };

        let created_user = match insert_user(conn, user_no_time, Some(auth_user.user.id)) {
            Ok(user) => user,
            Err(e) => {
                eprintln!("Error creating user: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Database error while creating user".to_string(),
                });
                return Err(response::status::Custom(Status::InternalServerError, err));
            }
        };

        // FOURTH: Assign roles to the user (roles already validated above)
        for role_name in &user_request.role_names {
            // Assign the role (we already validated everything above)
            if let Err(e) = assign_user_role_by_name(conn, created_user.id, role_name) {
                eprintln!("Error assigning role {}: {:?}", role_name, e);
                let err = Json(ErrorResponse {
                    error: format!("Failed to assign role '{}' to user", role_name),
                });
                return Err(response::status::Custom(Status::InternalServerError, err));
            }
        }

        // Get the user with roles after creation and role assignment
        match get_user_with_roles(conn, created_user.id) {
            Ok(Some(user_with_roles)) => Ok(status::Created::new("/").body(Json(user_with_roles))),
            Ok(None) => {
                let err = Json(ErrorResponse {
                    error: "User created but not found when retrieving with roles".to_string(),
                });
                Err(response::status::Custom(Status::InternalServerError, err))
            },
            Err(e) => {
                eprintln!("Error getting created user with roles: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "User created but failed to retrieve with roles".to_string(),
                });
                Err(response::status::Custom(Status::InternalServerError, err))
            }
        }
    })
    .await
}

/// List Users endpoint.
///
/// - **URL:** `/api/1/users`
/// - **Method:** `GET`
/// - **Purpose:** Retrieves all users in the system
/// - **Authentication:** Required
///
/// This endpoint retrieves all users from the database and returns them
/// as a JSON array. This includes all user information including timestamps
/// and associated company IDs.
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// [
///   {
///     "id": 1,
///     "email": "user1@example.com",
///     "password_hash": "hashed_password",
///     "company_id": 1,
///     "totp_secret": null,
///     "created_at": "2023-01-01T00:00:00Z",
///     "updated_at": "2023-01-01T00:00:00Z"
///   },
///   {
///     "id": 2,
///     "email": "user2@example.com",
///     "password_hash": "hashed_password",
///     "company_id": 2,
///     "totp_secret": "secret",
///     "created_at": "2023-01-01T00:00:00Z",
///     "updated_at": "2023-01-01T00:00:00Z"
///   }
/// ]
/// ```
///
/// # Arguments
/// * `db` - Database connection pool
///
/// # Returns
/// * `Ok(Json<Vec<User>>)` - List of all users
/// * `Err(Status)` - Error during retrieval (typically InternalServerError)
#[get("/1/Users?<query..>")]
pub async fn list_users(
    db: DbConn,
    auth_user: AuthenticatedUser,
    query: ODataQuery,
) -> Result<Json<serde_json::Value>, Status> {
    // Validate query options
    query.validate().map_err(|_| Status::BadRequest)?;
    
    // Authorization: determine which users this user can see
    let users = if auth_user.has_any_role(&["newtown-admin", "newtown-staff"]) {
        // newtown-admin and newtown-staff can see all users
        db.run(|conn| {
            list_all_users_with_roles(conn).map_err(|e| {
                eprintln!("Error listing all users: {:?}", e);
                Status::InternalServerError
            })
        })
        .await?
    } else if auth_user.has_role("admin") {
        // admin can only see users from their own company
        let company_id = auth_user.user.company_id;
        db.run(move |conn| {
            get_users_by_company_with_roles(conn, company_id).map_err(|e| {
                eprintln!("Error listing company users: {:?}", e);
                Status::InternalServerError
            })
        })
        .await?
    } else {
        // Regular users cannot list users
        return Err(Status::Forbidden);
    };

    // Apply filtering if specified
    let mut filtered_users = users;
    if let Some(filter_expr) = query.parse_filter() {
        // Basic filtering implementation - this could be expanded
        filtered_users = filtered_users
            .into_iter()
            .filter(|user| {
                // Simple implementation - could be much more sophisticated
                match &filter_expr.property.as_str() {
                    &"email" => match &filter_expr.value {
                        crate::odata_query::FilterValue::String(s) => match filter_expr.operator {
                            crate::odata_query::FilterOperator::Eq => user.email == *s,
                            crate::odata_query::FilterOperator::Ne => user.email != *s,
                            crate::odata_query::FilterOperator::Contains => user.email.contains(s),
                            _ => true,
                        },
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
                "email" => {
                    filtered_users.sort_by(|a, b| {
                        let cmp = a.email.cmp(&b.email);
                        match direction {
                            crate::odata_query::OrderDirection::Asc => cmp,
                            crate::odata_query::OrderDirection::Desc => cmp.reverse(),
                        }
                    });
                }
                "id" => {
                    filtered_users.sort_by(|a, b| {
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
    let total_count = filtered_users.len() as i64;

    // Apply skip and top
    if let Some(skip) = query.skip {
        filtered_users = filtered_users.into_iter().skip(skip as usize).collect();
    }
    if let Some(top) = query.top {
        filtered_users = filtered_users.into_iter().take(top as usize).collect();
    }

    // Handle $expand and computed properties, then $select
    let expand_props = query.parse_expand();
    let select_props = query.parse_select();
    let mut expanded_users: Vec<serde_json::Value> = Vec::new();
    
    // Check if activity timestamps are requested in $select
    let needs_activity_timestamps = if let Some(ref select_fields) = select_props {
        select_fields.iter().any(|field| 
            field == "activity_created_at" || field == "activity_updated_at"
        )
    } else {
        false // Default behavior doesn't include activity timestamps
    };
    
    for user in &filtered_users {
        let mut user_json = serde_json::to_value(user).map_err(|_| Status::InternalServerError)?;
        
        // Handle $expand=company
        if let Some(expansions) = &expand_props {
            if expansions.iter().any(|e| e.eq_ignore_ascii_case("company")) {
                // Load company data for this user
                let company_id = user.company_id;
                let company = db.run(move |conn| {
                    use crate::orm::company::get_company_by_id;
                    get_company_by_id(conn, company_id)
                }).await.map_err(|_| Status::InternalServerError)?;
                
                if let Some(company) = company {
                    user_json.as_object_mut()
                        .unwrap()
                        .insert("Company".to_string(), serde_json::to_value(company).map_err(|_| Status::InternalServerError)?);
                }
            }
        }
        
        // Handle computed activity timestamps if requested
        if needs_activity_timestamps {
            let user_id = user.id;
            let timestamps = db.run(move |conn| {
                use crate::orm::entity_activity::{get_created_at, get_updated_at};
                
                let created_at = get_created_at(conn, "users", user_id).ok();
                let updated_at = get_updated_at(conn, "users", user_id).ok();
                
                (created_at, updated_at)
            }).await;
            
            // Add activity timestamps to user object
            let user_obj = user_json.as_object_mut().unwrap();
            if let Some(created_at) = timestamps.0 {
                user_obj.insert("activity_created_at".to_string(), 
                    serde_json::Value::String(created_at.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()));
            } else {
                user_obj.insert("activity_created_at".to_string(), serde_json::Value::Null);
            }
            
            if let Some(updated_at) = timestamps.1 {
                user_obj.insert("activity_updated_at".to_string(), 
                    serde_json::Value::String(updated_at.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()));
            } else {
                user_obj.insert("activity_updated_at".to_string(), serde_json::Value::Null);
            }
        }
        
        expanded_users.push(user_json);
    }

    // Apply $select to each expanded user if specified
    let selected_users: Result<Vec<serde_json::Value>, _> = expanded_users
        .iter()
        .map(|user| apply_select(user, select_props.as_deref()))
        .collect();

    let selected_users = selected_users.map_err(|_| Status::InternalServerError)?;

    // Build OData response
    let context = build_context_url("http://localhost/api/1", "Users", select_props.as_deref());
    let mut response = ODataCollectionResponse::new(context, selected_users);

    // Add count if requested
    if query.count.unwrap_or(false) {
        response = response.with_count(total_count);
    }

    Ok(Json(serde_json::to_value(response).map_err(|_| Status::InternalServerError)?))
}

#[derive(serde::Deserialize)]
pub struct SetUserRoleRequest {
    pub user_id: i32,
    pub role_name: String,
}

/// Request structure for creating a user with roles.
#[derive(serde::Deserialize, serde::Serialize, TS)]
#[ts(export)]
pub struct CreateUserWithRolesRequest {
    pub email: String,
    pub password_hash: String,
    pub company_id: i32,
    pub totp_secret: Option<String>,
    pub role_names: Vec<String>,
}

/// Request structure for adding a role to a user (user_id comes from URL path).
#[derive(serde::Deserialize, TS)]
#[ts(export)]
pub struct AddUserRoleRequest {
    pub role_name: String,
}

/// Request structure for removing a role from a user (user_id comes from URL path).
#[derive(serde::Deserialize, TS)]
#[ts(export)]
pub struct RemoveUserRoleRequest {
    pub role_name: String,
}

/// Request structure for updating a user (all fields optional).
#[derive(serde::Deserialize, TS)]
#[ts(export)]
pub struct UpdateUserRequest {
    pub email: Option<String>,
    pub password_hash: Option<String>,
    pub company_id: Option<i32>,
    pub totp_secret: Option<String>,
}

/// Get User endpoint.
///
/// - **URL:** `/api/1/users/<user_id>`
/// - **Method:** `GET`
/// - **Purpose:** Retrieves a specific user by ID
/// - **Authentication:** Required
/// - **Authorization:** Users can view their own profile, admins can view any user
///
/// This endpoint retrieves a specific user's information. Users can view their
/// own profile data, while users with admin privileges can view any user's data.
///
/// # Parameters
///
/// - `user_id` - The ID of the user to retrieve
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// {
///   "id": 123,
///   "email": "user@example.com",
///   "password_hash": "hashed_password_string",
///   "company_id": 1,
///   "totp_secret": "optional_totp_secret",
///   "created_at": "2023-01-01T00:00:00Z",
///   "updated_at": "2023-01-01T00:00:00Z"
/// }
/// ```
///
/// **Failure (HTTP 403 Forbidden):**
/// User doesn't have permission to view the specified user
///
/// **Failure (HTTP 404 Not Found):**
/// User with specified ID doesn't exist
///
/// # Arguments
/// * `db` - Database connection pool
/// * `user_id` - The ID of the user to retrieve
/// * `auth_user` - The authenticated user making the request
///
/// # Returns
/// * `Ok(Json<User>)` - The requested user data
/// * `Err(response::status::Custom<Json<ErrorResponse>>)` - Error with JSON error details
#[get("/1/Users/<user_id>")]
pub async fn get_user_endpoint(
    db: DbConn,
    user_id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Json<UserWithRoles>, response::status::Custom<Json<ErrorResponse>>> {
    db.run(move |conn| {
        match get_user_with_roles(conn, user_id) {
            Ok(Some(user)) => {
                // Authorization: who can view this user?
                let can_view = if auth_user.user.id == user_id {
                    // Users can always view their own profile
                    true
                } else if auth_user.has_any_role(&["newtown-admin", "newtown-staff"]) {
                    // newtown-admin and newtown-staff can view any user
                    true
                } else if auth_user.has_role("admin") {
                    // Company admins can only view users from their own company
                    auth_user.user.company_id == user.company_id
                } else {
                    false
                };

                if !can_view {
                    let err = Json(ErrorResponse {
                        error: "Insufficient permissions to view this user".to_string(),
                    });
                    return Err(response::status::Custom(Status::Forbidden, err));
                }

                Ok(Json(user))
            }
            Ok(None) => {
                let err = Json(ErrorResponse {
                    error: "User not found".to_string(),
                });
                Err(response::status::Custom(Status::NotFound, err))
            }
            Err(diesel::result::Error::NotFound) => {
                let err = Json(ErrorResponse {
                    error: "User not found".to_string(),
                });
                Err(response::status::Custom(Status::NotFound, err))
            }
            Err(e) => {
                eprintln!("Error getting user: {:?}", e);
                let err = Json(ErrorResponse {
                    error: "Database error while retrieving user".to_string(),
                });
                Err(response::status::Custom(Status::InternalServerError, err))
            }
        }
    })
    .await
}

/// Get User Roles endpoint.
///
/// - **URL:** `/api/1/users/<user_id>/roles`
/// - **Method:** `GET`
/// - **Purpose:** Retrieves all roles assigned to a specific user
/// - **Authentication:** Required (users can view their own roles, or users with admin privileges can view any user's roles)
///
/// This endpoint retrieves all roles assigned to a specific user.
/// Users can view their own roles, or users with sufficient privileges
/// can view any user's roles.
///
/// # Parameters
///
/// - `user_id` - The ID of the user whose roles to retrieve
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// [
///   {
///     "id": 1,
///     "name": "admin",
///     "description": "Administrator role",
///     "created_at": "2023-01-01T00:00:00Z",
///     "updated_at": "2023-01-01T00:00:00Z"
///   },
///   {
///     "id": 2,
///     "name": "staff",
///     "description": "Staff role",
///     "created_at": "2023-01-01T00:00:00Z",
///     "updated_at": "2023-01-01T00:00:00Z"
///   }
/// ]
/// ```
///
/// **Failure (HTTP 403 Forbidden):**
/// User doesn't have permission to view the specified user's roles
///
/// # Arguments
/// * `db` - Database connection pool
/// * `user_id` - The ID of the user whose roles to retrieve
/// * `auth_user` - The authenticated user making the request
///
/// # Returns
/// * `Ok(Json<Vec<Role>>)` - List of roles for the specified user
/// * `Err(Status)` - Error status (Forbidden, InternalServerError, etc.)
///
/// # Example
///
/// ```js
/// const response = await fetch('/api/1/users/123/roles', {
///   method: 'GET',
///   credentials: 'include'
/// });
/// ```
#[get("/1/Users/<user_id>/Roles")]
pub async fn get_user_roles_endpoint(
    db: DbConn,
    user_id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Json<Vec<Role>>, Status> {
    // Users can view their own roles, admins can view any user's roles
    if auth_user.user.id != user_id
        && !auth_user.has_any_role(&["newtown-admin", "newtown-staff", "admin"])
    {
        return Err(Status::Forbidden);
    }

    db.run(move |conn| {
        get_user_roles(conn, user_id).map(Json).map_err(|e| {
            eprintln!("Error getting user roles: {:?}", e);
            Status::InternalServerError
        })
    })
    .await
}

/// Add User Role endpoint.
///
/// - **URL:** `/api/1/users/<user_id>/roles`
/// - **Method:** `POST`
/// - **Purpose:** Assigns a role to a user with authorization checks
/// - **Authentication:** Required (admin privileges with specific business rules)
///
/// This endpoint allows authorized users to add roles to other users
/// following the business rules:
/// 1. `newtown-staff` and `newtown-admin` roles are reserved for Newtown Energy company
/// 2. `newtown-admin` can set any user's role to anything
/// 3. `newtown-staff` can set any user's role except `newtown-admin`
/// 4. `admin` can set another user's role to `admin` if target user is at same company
/// 5. Users must have at least one role (validated elsewhere)
///
/// # Authorization Rules
///
/// 1. `newtown-staff` and `newtown-admin` roles are reserved for Newtown Energy company
/// 2. `newtown-admin` can set any user's role to anything
/// 3. `newtown-staff` can set any user's role except `newtown-admin`
/// 4. `admin` can set another user's role to `admin` if target user is at same company
/// 5. Users must have at least one role (validated elsewhere)
///
/// # Request Format
///
/// ```json
/// {
///   "role_name": "staff"
/// }
/// ```
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// No response body - role successfully assigned
///
/// **Failure (HTTP 403 Forbidden):**
/// User doesn't have permission to assign the specified role
///
/// **Failure (HTTP 500 Internal Server Error):**
/// Database error or validation failure
///
/// # Arguments
/// * `db` - Database connection pool
/// * `user_id` - User ID from URL path parameter
/// * `request` - JSON payload containing role_name to add
/// * `auth_user` - The authenticated user making the request
///
/// # Returns
/// * `Ok(Status::Ok)` - Role successfully assigned
/// * `Err(Status)` - Error status (Forbidden, InternalServerError, etc.)
///
/// # Example
///
/// ```js
/// const response = await fetch('/api/1/users/123/roles', {
///   method: 'POST',
///   headers: { 'Content-Type': 'application/json' },
///   body: JSON.stringify({
///     role_name: 'staff'
///   }),
///   credentials: 'include'
/// });
/// ```
#[post("/1/Users/<user_id>/Roles", data = "<request>")]
pub async fn add_user_role(
    db: DbConn,
    user_id: i32,
    request: Json<AddUserRoleRequest>,
    auth_user: AuthenticatedUser,
) -> Result<Status, Status> {
    let target_user_id = user_id;
    let role_name = request.role_name.clone();

    // Get target user's company for validation
    let target_user = db
        .run(move |conn| get_user(conn, target_user_id))
        .await
        .map_err(|e| {
            eprintln!("Error getting target user: {:?}", e);
            Status::InternalServerError
        })?
        .ok_or(Status::NotFound)?;

    // Authorization check based on business rules
    let can_assign = if auth_user.has_role("newtown-admin") {
        // Rule 2: newtown-admin can set any user's role to anything
        true
    } else if auth_user.has_role("newtown-staff") {
        // Rule 3: newtown-staff can set any user's role except newtown-admin
        role_name != "newtown-admin"
    } else if auth_user.has_role("admin") {
        // Rule 4: admin can set another user's role to any role if same company
        auth_user.user.company_id == target_user.company_id
    } else {
        false
    };

    if !can_assign {
        return Err(Status::Forbidden);
    }

    // Rule 1: newtown-staff and newtown-admin roles are reserved for Newtown Energy
    if role_name == "newtown-staff" || role_name == "newtown-admin" {
        let newtown_company_search = CompanyInput {
            name: "Newtown Energy".to_string(),
        };
        let newtown_company = db
            .run(move |conn| get_company_by_name(conn, &newtown_company_search))
            .await
            .map_err(|e| {
                eprintln!("Error getting Newtown Energy company: {:?}", e);
                Status::InternalServerError
            })?;

        let newtown_company = match newtown_company {
            Some(inst) => inst,
            None => {
                eprintln!("Newtown Energy company not found");
                return Err(Status::InternalServerError);
            }
        };

        if target_user.company_id != newtown_company.id {
            return Err(Status::Forbidden);
        }
    }

    // Assign the role
    db.run(move |conn| {
        assign_user_role_by_name(conn, target_user_id, &role_name).map_err(|e| {
            eprintln!("Error assigning user role: {:?}", e);
            Status::InternalServerError
        })
    })
    .await?;

    Ok(Status::Ok)
}

/// Remove User Role endpoint.
///
/// - **URL:** `/api/1/users/<user_id>/roles`
/// - **Method:** `DELETE`
/// - **Purpose:** Removes a role from a user with authorization checks
/// - **Authentication:** Required (same authorization rules as adding roles)
///
/// This endpoint allows authorized users to remove roles from other users
/// following the same authorization rules as adding roles. Additionally,
/// it ensures users always retain at least one role.
///
/// # Authorization Rules
///
/// Same authorization rules as adding roles, plus:
/// - Users must retain at least one role after removal
///
/// # Request Format
///
/// ```json
/// {
///   "role_name": "staff"
/// }
/// ```
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// No response body - role successfully removed
///
/// **Failure (HTTP 400 Bad Request):**
/// User would have no roles remaining after removal
///
/// **Failure (HTTP 403 Forbidden):**
/// User doesn't have permission to remove the specified role
///
/// **Failure (HTTP 500 Internal Server Error):**
/// Database error or validation failure
///
/// # Arguments
/// * `db` - Database connection pool
/// * `user_id` - User ID from URL path parameter
/// * `request` - JSON payload containing role_name to remove
/// * `auth_user` - The authenticated user making the request
///
/// # Returns
/// * `Ok(Status::Ok)` - Role successfully removed
/// * `Err(Status)` - Error status (Forbidden, BadRequest, InternalServerError, etc.)
///
/// # Example
///
/// ```js
/// const response = await fetch('/api/1/users/123/roles', {
///   method: 'DELETE',
///   headers: { 'Content-Type': 'application/json' },
///   body: JSON.stringify({
///     role_name: 'staff'
///   }),
///   credentials: 'include'
/// });
/// ```
#[delete("/1/Users/<user_id>/Roles", data = "<request>")]
pub async fn remove_user_role(
    db: DbConn,
    user_id: i32,
    request: Json<RemoveUserRoleRequest>,
    auth_user: AuthenticatedUser,
) -> Result<Status, Status> {
    let target_user_id = user_id;
    let role_name = request.role_name.clone();

    // Get target user's company for validation
    let target_user = db
        .run(move |conn| get_user(conn, target_user_id))
        .await
        .map_err(|e| {
            eprintln!("Error getting target user: {:?}", e);
            Status::InternalServerError
        })?
        .ok_or(Status::NotFound)?;

    // Check if user would have any roles left after removal
    let current_roles = db
        .run(move |conn| get_user_roles(conn, target_user_id))
        .await
        .map_err(|e| {
            eprintln!("Error getting current user roles: {:?}", e);
            Status::InternalServerError
        })?;

    // Rule 5: Users must have at least one role
    if current_roles.len() <= 1 {
        return Err(Status::BadRequest);
    }

    // Authorization check - same rules as adding roles
    let can_remove = if auth_user.has_role("newtown-admin") {
        true
    } else if auth_user.has_role("newtown-staff") {
        role_name != "newtown-admin"
    } else if auth_user.has_role("admin") {
        auth_user.user.company_id == target_user.company_id
    } else {
        false
    };

    if !can_remove {
        return Err(Status::Forbidden);
    }

    // Remove the role
    db.run(move |conn| {
        remove_user_role_by_name(conn, target_user_id, &role_name).map_err(|e| {
            eprintln!("Error removing user role: {:?}", e);
            Status::InternalServerError
        })
    })
    .await?;

    Ok(Status::Ok)
}

/// Update User endpoint.
///
/// - **URL:** `/api/1/users/<user_id>`
/// - **Method:** `PUT`
/// - **Purpose:** Updates a user's information
/// - **Authentication:** Required
/// - **Authorization:** Users can update their own profile, admins can update any user
///
/// This endpoint allows updating user information. Users can update their own
/// profile data, while users with admin privileges can update any user's data.
/// All fields in the request are optional - only provided fields will be updated.
///
/// # Parameters
///
/// - `user_id` - The ID of the user to update
///
/// # Request Format
///
/// ```json
/// {
///   "email": "newemail@example.com",
///   "password_hash": "new_hashed_password",
///   "company_id": 2,
///   "totp_secret": "new_totp_secret"
/// }
/// ```
///
/// # Response
///
/// **Success (HTTP 200 OK):**
/// ```json
/// {
///   "id": 123,
///   "email": "newemail@example.com",
///   "password_hash": "new_hashed_password",
///   "company_id": 2,
///   "totp_secret": "new_totp_secret",
///   "created_at": "2023-01-01T00:00:00Z",
///   "updated_at": "2023-01-01T12:30:00Z"
/// }
/// ```
///
/// **Failure (HTTP 403 Forbidden):**
/// User doesn't have permission to update the specified user
///
/// **Failure (HTTP 404 Not Found):**
/// User with specified ID doesn't exist
///
/// # Arguments
/// * `db` - Database connection pool
/// * `user_id` - The ID of the user to update
/// * `request` - JSON payload containing fields to update
/// * `auth_user` - The authenticated user making the request
///
/// # Returns
/// * `Ok(Json<User>)` - The updated user data
/// * `Err(Status)` - Error status (Forbidden, NotFound, InternalServerError)
#[put("/1/Users/<user_id>", data = "<request>")]
pub async fn update_user_endpoint(
    db: DbConn,
    user_id: i32,
    request: Json<UpdateUserRequest>,
    auth_user: AuthenticatedUser,
) -> Result<Json<UserWithRoles>, Status> {
    db.run(move |conn| {
        // First, get the target user to check authorization
        let target_user = match get_user(conn, user_id) {
            Ok(Some(user)) => user,
            Ok(None) => return Err(Status::NotFound),
            Err(diesel::result::Error::NotFound) => return Err(Status::NotFound),
            Err(e) => {
                eprintln!("Error getting user for update: {:?}", e);
                return Err(Status::InternalServerError);
            }
        };

        // Authorization: who can update this user?
        let can_update = if auth_user.user.id == user_id {
            // Users can always update their own profile
            true
        } else if auth_user.has_any_role(&["newtown-admin", "newtown-staff"]) {
            // newtown-admin and newtown-staff can update any user
            true
        } else if auth_user.has_role("admin") {
            // Company admins can only update users from their own company
            auth_user.user.company_id == target_user.company_id
        } else {
            false
        };

        if !can_update {
            return Err(Status::Forbidden);
        }

        match update_user(
            conn,
            user_id,
            request.email.clone(),
            request.password_hash.clone(),
            request.company_id,
            request.totp_secret.clone(),
            Some(auth_user.user.id),
        ) {
            Ok(_user) => {
                // Get the updated user with roles
                match get_user_with_roles(conn, user_id) {
                    Ok(Some(user_with_roles)) => Ok(Json(user_with_roles)),
                    Ok(None) => Err(Status::NotFound),
                    Err(e) => {
                        eprintln!("Error getting updated user with roles: {:?}", e);
                        Err(Status::InternalServerError)
                    }
                }
            }
            Err(diesel::result::Error::NotFound) => Err(Status::NotFound),
            Err(e) => {
                eprintln!("Error updating user: {:?}", e);
                Err(Status::InternalServerError)
            }
        }
    })
    .await
}

/// Delete User endpoint.
///
/// - **URL:** `/api/1/users/<user_id>`
/// - **Method:** `DELETE`
/// - **Purpose:** Deletes a user from the system
/// - **Authentication:** Required
/// - **Authorization:** Only newtown-admin and newtown-staff can delete users
///
/// This endpoint permanently removes a user from the system. This is a
/// destructive operation that also removes associated data like user roles
/// and sessions. Only users with newtown-admin or newtown-staff roles can
/// delete users.
///
/// **Warning**: This is a hard delete operation that cannot be undone.
///
/// # Parameters
///
/// - `user_id` - The ID of the user to delete
///
/// # Response
///
/// **Success (HTTP 204 No Content):**
/// No response body - user successfully deleted
///
/// **Failure (HTTP 403 Forbidden):**
/// User doesn't have permission to delete users
///
/// **Failure (HTTP 404 Not Found):**
/// User with specified ID doesn't exist
///
/// # Arguments
/// * `db` - Database connection pool
/// * `user_id` - The ID of the user to delete
/// * `auth_user` - The authenticated user making the request
///
/// # Returns
/// * `Ok(Status::NoContent)` - User successfully deleted
/// * `Err(Status)` - Error status (Forbidden, NotFound, InternalServerError)
#[delete("/1/Users/<user_id>")]
pub async fn delete_user_endpoint(
    db: DbConn,
    user_id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Status, Status> {
    db.run(move |conn| {
        // First, get the target user to check authorization
        let target_user = match get_user(conn, user_id) {
            Ok(Some(user)) => user,
            Ok(None) => return Err(Status::NotFound),
            Err(diesel::result::Error::NotFound) => return Err(Status::NotFound),
            Err(e) => {
                eprintln!("Error getting user for deletion: {:?}", e);
                return Err(Status::InternalServerError);
            }
        };

        // Authorization: who can delete this user?
        let can_delete = if auth_user.has_any_role(&["newtown-admin", "newtown-staff"]) {
            // newtown-admin and newtown-staff can delete any user
            true
        } else if auth_user.has_role("admin") {
            // Company admins can only delete users from their own company
            auth_user.user.company_id == target_user.company_id
        } else {
            false
        };

        if !can_delete {
            return Err(Status::Forbidden);
        }

        match delete_user_with_cleanup(conn, user_id, Some(auth_user.user.id)) {
            Ok(rows_affected) => {
                if rows_affected > 0 {
                    Ok(Status::NoContent)
                } else {
                    Err(Status::NotFound)
                }
            }
            Err(e) => {
                eprintln!("Error deleting user: {:?}", e);
                Err(Status::InternalServerError)
            }
        }
    })
    .await
}

/// Get User Company Navigation endpoint.
///
/// - **URL:** `/api/1/Users/<user_id>/Company`
/// - **Method:** `GET`  
/// - **Purpose:** Retrieves the company associated with a user (OData navigation property)
/// - **Authentication:** Required
///
/// This is an OData navigation endpoint that returns the Company entity
/// associated with the specified user.
#[get("/1/Users/<user_id>/Company")]
pub async fn get_user_company(
    db: DbConn,
    user_id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Json<crate::models::Company>, Status> {
    // Authorization check: same as getting a user
    let target_user = db.run(move |conn| {
        get_user(conn, user_id)
    }).await.map_err(|_| Status::InternalServerError)?;
    
    let target_user = target_user.ok_or(Status::NotFound)?;
    
    let can_view = if auth_user.user.id == user_id {
        true
    } else if auth_user.has_any_role(&["newtown-admin", "newtown-staff"]) {
        true
    } else if auth_user.has_role("admin") {
        auth_user.user.company_id == target_user.company_id
    } else {
        false
    };

    if !can_view {
        return Err(Status::Forbidden);
    }

    // Get the company
    let company_id = target_user.company_id;
    db.run(move |conn| {
        use crate::orm::company::get_company_by_id;
        get_company_by_id(conn, company_id)
    })
    .await
    .map_err(|_| Status::InternalServerError)?
    .map(Json)
    .ok_or(Status::NotFound)
}

/// Get User Roles Navigation endpoint.
///
/// - **URL:** `/api/1/Users/<user_id>/Roles`
/// - **Method:** `GET`  
/// - **Purpose:** Retrieves the roles associated with a user (OData navigation property)
/// - **Authentication:** Required
///
/// This is an OData navigation endpoint that returns the Role entities
/// associated with the specified user. This is the same as get_user_roles_endpoint
/// but follows OData navigation conventions.
// Note: This endpoint is already implemented as get_user_roles_endpoint above

/// Returns a vector of all routes defined in this module.
///
/// This function collects all the route handlers defined in this module
/// and returns them as a vector for registration with the Rocket framework.
///
/// # Returns
/// A vector containing all route handlers for user endpoints
pub fn routes() -> Vec<Route> {
    routes![
        create_user,
        list_users,
        get_user_endpoint,
        update_user_endpoint,
        delete_user_endpoint,
        get_user_roles_endpoint,
        add_user_role,
        remove_user_role,
        get_user_company
    ]
}
