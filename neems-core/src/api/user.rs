//! API endpoints for managing users.
//!
//! This module provides HTTP endpoints for creating and listing users,
//! along with utility functions for generating test data and helper functions
//! for API testing.

use rand::rng;
use rand::prelude::IndexedRandom;
use rocket::http::{ContentType, Status};
use rocket::local::asynchronous::Client;
use rocket::response::status;
use rocket::Route;
use rocket::serde::json::{json, Json};

use crate::session_guards::AuthenticatedUser;
use crate::orm::DbConn;
use crate::orm::user::{insert_user, list_all_users, get_user, update_user, get_users_by_company, delete_user_with_cleanup};
use crate::orm::user_role::{get_user_roles, assign_user_role_by_name, remove_user_role_by_name};
use crate::orm::company::get_company_by_name;
use crate::models::{User, UserNoTime, Role, CompanyNoTime};

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
	"a.johnson", "b.williams", "c.miller", "d.davis", "e.rodriguez",
	"f.martinez", "g.lee", "h.wilson", "i.clark", "j.hernandez",
	"k.young", "l.walker", "m.hall", "n.allen", "o.green", "p.adams",
	"q.nelson", "r.mitchell", "s.carter", "t.roberts", "amandak",
	"brandonp", "chrisl", "davidm", "ericb", "frankr", "garys",
	"heathert", "ianw", "jenniferg", "kevinh", "lisac", "michaeld",
	"nicolef", "oliverj", "patrickt", "quincyv", "rachelm", "stevenn",
	"taylorq", "jameskw1", "sarahml2", "robertdf3", "laurajg4",
	"thomasap5", "emilyrs6", "danielkt7", "megandw8", "ryanbh9",
	"oliviamc10", "aljohnson", "bkmartin", "cjwilson", "dlthomas",
	"emharris", "fnmoore", "gpgarcia", "hrjackson", "iswhite", "jdtaylor",
	"browns", "moorej", "evansm", "kingr", "wrighta", "scottl", "riverak",
	"hayesd", "collinsp", "murphyb", "mikescott", "aligray", "chrismyers",
	"jenngreen", "robhall", "davecook", "sarahkim", "timnguyen",
	"katediaz", "jimreed", "analyst_amy", "director_mark", "manager_lisa",
	"tech_sam", "scientist_raj", "ops_carlos", "ceo_adam", "cto_priya",
	"designer_tom", "specialist_lee", "wind_mike", "nuclear_dave",
	"battery_lucy", "grid_omar", "fusion_anna", "hydro_ryan",
	"solar_priya", "storage_paul", "transmission_ella", "renewables_jack",
	"a.kumar24", "b.liang2024", "c.patel_eng", "d.yang_ops",
	"e.choi_tech", "f.singh1", "g.wu2023", "h.garcia_ce", "i.vargas_pe",
	"j.nguyen_lead", "alexclark", "briancook", "carolynlee", "davidbrown",
	"ericawang", "franklinm", "gracehill", "henryford", "ivyzhang",
	"jasonpark", "volts_ryan", "amp_anne", "watt_dan", "joule_mary",
	"ohm_steve", "grid_master", "solar_expert", "wind_tech", "nuke_ops",
	"fusion_research", "battery_ai", "smartgrid_pro", "renewables_lead",
	"carbon_zero", "green_volt", "energy_analyst", "power_engineer",
	"grid_designer", "sustainability_1", "clean_energy_22", "ceo_johnson",
	"cfo_smith", "cto_lee", "vp_operations", "director_energy", "head_rd",
	"manager_grid", "lead_engineer", "senior_designer", "principal_tech",
	"engineer1", "systems_ops", "grid_analyst", "nuke_specialist",
	"solar_tech", "wind_engineer", "battery_design", "transmission_pro",
	"power_ops", "fusion_researcher", "hr_jane", "finance_mike",
	"legal_lisa", "admin_alex", "it_support", "comms_dan", "pr_sarah",
	"facilities_tom", "security_lead", "logistics_team", "jdoe_energy",
	"asmith_power", "rlee_solar", "kwang_grid", "tchen_nuke",
	"lrod_fusion", "pmartin_wind", "sgarcia_storage", "dwilson_ops",
	"ajames_ce", "bkim_tech", "clopez_eng", "dhall_design",
	"eyoung_analyst", "fscott_lead", "gadams_rd", "hbaker_sys",
	"igray_ai", "jflores_data", "kharris_coo", "lmurphy_cfo",
	"mrivera_cto", "npham_vp", "opark_dir", "pcole_mgr", "qedwards_hr",
	"rfoster_fin", "snguyen_legal", "tross_it", "upatel_admin"
    ];
    let mut rng = rng();
    let selected: Vec<_> = names.choose_multiple(&mut rng, count).copied().collect();
    selected
}

/// Helper function to create a user via the API and return the created User.
///
/// This function is primarily used for testing purposes. It makes a POST request
/// to the user creation endpoint and returns the newly created user object.
///
/// # Arguments
/// * `client` - The Rocket test client instance
/// * `user` - The user data to create (without timestamp fields)
///
/// # Returns
/// The created User object with all fields populated
///
/// # Panics
/// This function will panic if the API request fails or returns invalid data,
/// as it's intended for testing scenarios where such failures indicate test problems.
pub async fn create_user_by_api(
    client: &Client,
    user: &UserNoTime,
) -> User {
    let body = json!({
        "email": &user.email,
        "password_hash": &user.password_hash,
        "company_id": user.company_id,
        "totp_secret": &user.totp_secret
    }).to_string();
    let response = client
        .post("/api/1/users")
        .header(ContentType::JSON)
        .body(body)
        .dispatch()
        .await;

    assert_eq!(response.status(), rocket::http::Status::Created);

    response
        .into_json::<User>()
        .await
        .expect("valid User JSON response")
}

/// Create User endpoint.
///
/// - **URL:** `/api/1/users`
/// - **Method:** `POST`
/// - **Purpose:** Creates a new user in the system
/// - **Authentication:** Required
///
/// This endpoint accepts a JSON payload containing user information and
/// creates a new user record in the database. The user data should not
/// include timestamp fields as they are automatically generated.
///
/// # Request Format
///
/// ```json
/// {
///   "email": "newuser@example.com",
///   "password_hash": "hashed_password_string",
///   "company_id": 1,
///   "totp_secret": "optional_totp_secret"
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
///   "updated_at": "2023-01-01T00:00:00Z"
/// }
/// ```
///
/// **Failure (HTTP 500 Internal Server Error):**
/// Database error or validation failure
///
/// # Arguments
/// * `db` - Database connection pool
/// * `new_user` - JSON payload containing the new user data
///
/// # Returns
/// * `Ok(status::Created<Json<User>>)` - Successfully created user
/// * `Err(Status)` - Error during creation (typically InternalServerError)
#[post("/1/users", data = "<new_user>")]
pub async fn create_user(
    db: DbConn,
    new_user: Json<UserNoTime>,
    auth_user: AuthenticatedUser
) -> Result<status::Created<Json<User>>, Status> {
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
        return Err(Status::Forbidden);
    }

    db.run(move |conn| {
        insert_user(conn, new_user.into_inner())
            .map(|user| status::Created::new("/").body(Json(user)))
            .map_err(|e| {
                eprintln!("Error creating user: {:?}", e);
                Status::InternalServerError
            })
    }).await
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
#[get("/1/users")]
pub async fn list_users(
    db: DbConn,
    auth_user: AuthenticatedUser
) -> Result<Json<Vec<User>>, Status> {
    // Authorization: determine which users this user can see
    if auth_user.has_any_role(&["newtown-admin", "newtown-staff"]) {
        // newtown-admin and newtown-staff can see all users
        db.run(|conn| {
            list_all_users(conn)
                .map(Json)
                .map_err(|e| {
                    eprintln!("Error listing all users: {:?}", e);
                    Status::InternalServerError
                })
        }).await
    } else if auth_user.has_role("admin") {
        // admin can only see users from their own company
        let company_id = auth_user.user.company_id;
        db.run(move |conn| {
            get_users_by_company(conn, company_id)
                .map(Json)
                .map_err(|e| {
                    eprintln!("Error listing company users: {:?}", e);
                    Status::InternalServerError
                })
        }).await
    } else {
        // Regular users cannot list users
        Err(Status::Forbidden)
    }
}

#[derive(serde::Deserialize)]
pub struct SetUserRoleRequest {
    pub user_id: i32,
    pub role_name: String,
}

/// Request structure for adding a role to a user (user_id comes from URL path).
#[derive(serde::Deserialize)]
pub struct AddUserRoleRequest {
    pub role_name: String,
}

/// Request structure for removing a role from a user (user_id comes from URL path).
#[derive(serde::Deserialize)]
pub struct RemoveUserRoleRequest {
    pub role_name: String,
}

/// Request structure for updating a user (all fields optional).
#[derive(serde::Deserialize)]
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
/// * `Err(Status)` - Error status (Forbidden, NotFound, InternalServerError)
#[get("/1/users/<user_id>")]
pub async fn get_user_endpoint(
    db: DbConn,
    user_id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Json<User>, Status> {
    db.run(move |conn| {
        match get_user(conn, user_id) {
            Ok(user) => {
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
                    return Err(Status::Forbidden);
                }
                
                Ok(Json(user))
            },
            Err(diesel::result::Error::NotFound) => Err(Status::NotFound),
            Err(e) => {
                eprintln!("Error getting user: {:?}", e);
                Err(Status::InternalServerError)
            }
        }
    }).await
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
#[get("/1/users/<user_id>/roles")]
pub async fn get_user_roles_endpoint(
    db: DbConn,
    user_id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Json<Vec<Role>>, Status> {
    // Users can view their own roles, admins can view any user's roles
    if auth_user.user.id != user_id && 
       !auth_user.has_any_role(&["newtown-admin", "newtown-staff", "admin"]) {
        return Err(Status::Forbidden);
    }

    db.run(move |conn| {
        get_user_roles(conn, user_id)
            .map(Json)
            .map_err(|e| {
                eprintln!("Error getting user roles: {:?}", e);
                Status::InternalServerError
            })
    }).await
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
#[post("/1/users/<user_id>/roles", data = "<request>")]
pub async fn add_user_role(
    db: DbConn,
    user_id: i32,
    request: Json<AddUserRoleRequest>,
    auth_user: AuthenticatedUser,
) -> Result<Status, Status> {
    let target_user_id = user_id;
    let role_name = request.role_name.clone();

    // Get target user's company for validation
    let target_user = db.run(move |conn| {
        get_user(conn, target_user_id)
    }).await.map_err(|e| {
        eprintln!("Error getting target user: {:?}", e);
        Status::InternalServerError
    })?;

    // Authorization check based on business rules
    let can_assign = if auth_user.has_role("newtown-admin") {
        // Rule 2: newtown-admin can set any user's role to anything
        true
    } else if auth_user.has_role("newtown-staff") {
        // Rule 3: newtown-staff can set any user's role except newtown-admin
        role_name != "newtown-admin"
    } else if auth_user.has_role("admin") {
        // Rule 4: admin can set another user's role to admin if same company
        role_name == "admin" && auth_user.user.company_id == target_user.company_id
    } else {
        false
    };

    if !can_assign {
        return Err(Status::Forbidden);
    }

    // Rule 1: newtown-staff and newtown-admin roles are reserved for Newtown Energy
    if role_name == "newtown-staff" || role_name == "newtown-admin" {
        let newtown_company_search = CompanyNoTime {
            name: "Newtown Energy".to_string(),
        };
        let newtown_company = db.run(move |conn| {
            get_company_by_name(conn, &newtown_company_search)
        }).await.map_err(|e| {
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
        assign_user_role_by_name(conn, target_user_id, &role_name)
            .map_err(|e| {
                eprintln!("Error assigning user role: {:?}", e);
                Status::InternalServerError
            })
    }).await?;

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
#[delete("/1/users/<user_id>/roles", data = "<request>")]
pub async fn remove_user_role(
    db: DbConn,
    user_id: i32,
    request: Json<RemoveUserRoleRequest>,
    auth_user: AuthenticatedUser,
) -> Result<Status, Status> {
    let target_user_id = user_id;
    let role_name = request.role_name.clone();

    // Get target user's company for validation
    let target_user = db.run(move |conn| {
        get_user(conn, target_user_id)
    }).await.map_err(|e| {
        eprintln!("Error getting target user: {:?}", e);
        Status::InternalServerError
    })?;

    // Check if user would have any roles left after removal
    let current_roles = db.run(move |conn| {
        get_user_roles(conn, target_user_id)
    }).await.map_err(|e| {
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
        role_name == "admin" && auth_user.user.company_id == target_user.company_id
    } else {
        false
    };

    if !can_remove {
        return Err(Status::Forbidden);
    }

    // Remove the role
    db.run(move |conn| {
        remove_user_role_by_name(conn, target_user_id, &role_name)
            .map_err(|e| {
                eprintln!("Error removing user role: {:?}", e);
                Status::InternalServerError
            })
    }).await?;

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
#[put("/1/users/<user_id>", data = "<request>")]
pub async fn update_user_endpoint(
    db: DbConn,
    user_id: i32,
    request: Json<UpdateUserRequest>,
    auth_user: AuthenticatedUser,
) -> Result<Json<User>, Status> {
    db.run(move |conn| {
        // First, get the target user to check authorization
        let target_user = match get_user(conn, user_id) {
            Ok(user) => user,
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
        ) {
            Ok(user) => Ok(Json(user)),
            Err(diesel::result::Error::NotFound) => Err(Status::NotFound),
            Err(e) => {
                eprintln!("Error updating user: {:?}", e);
                Err(Status::InternalServerError)
            }
        }
    }).await
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
#[delete("/1/users/<user_id>")]
pub async fn delete_user_endpoint(
    db: DbConn,
    user_id: i32,
    auth_user: AuthenticatedUser,
) -> Result<Status, Status> {
    db.run(move |conn| {
        // First, get the target user to check authorization
        let target_user = match get_user(conn, user_id) {
            Ok(user) => user,
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
        
        match delete_user_with_cleanup(conn, user_id) {
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
    }).await
}

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
        remove_user_role
    ]
}