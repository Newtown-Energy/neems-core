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
use crate::orm::user::{insert_user, list_all_users, get_user};
use crate::orm::user_role::{get_user_roles, assign_user_role_by_name, remove_user_role_by_name};
use crate::orm::institution::get_institution_by_name;
use crate::models::{User, UserNoTime, Role, InstitutionNoTime};

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
        "institution_id": user.institution_id,
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

/// Creates a new user in the system.
///
/// This endpoint accepts a JSON payload containing user information and
/// creates a new user record in the database. The user data should not
/// include timestamp fields as they are automatically generated.
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
    _auth_user: AuthenticatedUser
) -> Result<status::Created<Json<User>>, Status> {
    db.run(move |conn| {
        insert_user(conn, new_user.into_inner())
            .map(|user| status::Created::new("/").body(Json(user)))
            .map_err(|e| {
                eprintln!("Error creating user: {:?}", e);
                Status::InternalServerError
            })
    }).await
}

/// Lists all users in the system.
///
/// This endpoint retrieves all users from the database and returns them
/// as a JSON array. This includes all user information including timestamps
/// and associated institution IDs.
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
    _auth_user: AuthenticatedUser
) -> Result<Json<Vec<User>>, Status> {
    db.run(|conn| {
        list_all_users(conn)
            .map(Json)
            .map_err(|e| {
                eprintln!("Error listing users: {:?}", e);
                Status::InternalServerError
            })
    }).await
}

#[derive(serde::Deserialize)]
pub struct SetUserRoleRequest {
    pub user_id: i32,
    pub role_name: String,
}

/// Gets the roles for a specific user.
///
/// This endpoint retrieves all roles assigned to a specific user.
/// Users can view their own roles, or users with sufficient privileges
/// can view any user's roles.
///
/// # Arguments
/// * `db` - Database connection pool
/// * `user_id` - The ID of the user whose roles to retrieve
/// * `auth_user` - The authenticated user making the request
///
/// # Returns
/// * `Ok(Json<Vec<Role>>)` - List of roles for the specified user
/// * `Err(Status)` - Error status (Forbidden, InternalServerError, etc.)
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

/// Sets a user's role with authorization checks.
///
/// This endpoint allows authorized users to add roles to other users
/// following the business rules:
/// 1. newtown-staff and newtown-admin roles are reserved for Newtown Energy institution
/// 2. newtown-admin can set any user's role to anything
/// 3. newtown-staff can set any user's role except newtown-admin
/// 4. admin can set another user's role to admin if target user is at same institution
/// 5. Users must have at least one role (validated elsewhere)
///
/// # Arguments
/// * `db` - Database connection pool
/// * `request` - JSON payload containing user_id and role_name to add
/// * `auth_user` - The authenticated user making the request
///
/// # Returns
/// * `Ok(Status::Ok)` - Role successfully assigned
/// * `Err(Status)` - Error status (Forbidden, InternalServerError, etc.)
#[post("/1/users/roles", data = "<request>")]
pub async fn add_user_role(
    db: DbConn,
    request: Json<SetUserRoleRequest>,
    auth_user: AuthenticatedUser,
) -> Result<Status, Status> {
    let target_user_id = request.user_id;
    let role_name = request.role_name.clone();

    // Get target user's institution for validation
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
        // Rule 4: admin can set another user's role to admin if same institution
        role_name == "admin" && auth_user.user.institution_id == target_user.institution_id
    } else {
        false
    };

    if !can_assign {
        return Err(Status::Forbidden);
    }

    // Rule 1: newtown-staff and newtown-admin roles are reserved for Newtown Energy
    if role_name == "newtown-staff" || role_name == "newtown-admin" {
        let newtown_institution_search = InstitutionNoTime {
            name: "Newtown Energy".to_string(),
        };
        let newtown_institution = db.run(move |conn| {
            get_institution_by_name(conn, &newtown_institution_search)
        }).await.map_err(|e| {
            eprintln!("Error getting Newtown Energy institution: {:?}", e);
            Status::InternalServerError
        })?;

        let newtown_institution = match newtown_institution {
            Some(inst) => inst,
            None => {
                eprintln!("Newtown Energy institution not found");
                return Err(Status::InternalServerError);
            }
        };

        if target_user.institution_id != newtown_institution.id {
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

/// Removes a role from a user with authorization checks.
///
/// This endpoint allows authorized users to remove roles from other users
/// following the same authorization rules as adding roles. Additionally,
/// it ensures users always retain at least one role.
///
/// # Arguments
/// * `db` - Database connection pool
/// * `request` - JSON payload containing user_id and role_name to remove
/// * `auth_user` - The authenticated user making the request
///
/// # Returns
/// * `Ok(Status::Ok)` - Role successfully removed
/// * `Err(Status)` - Error status (Forbidden, BadRequest, InternalServerError, etc.)
#[delete("/1/users/roles", data = "<request>")]
pub async fn remove_user_role(
    db: DbConn,
    request: Json<SetUserRoleRequest>,
    auth_user: AuthenticatedUser,
) -> Result<Status, Status> {
    let target_user_id = request.user_id;
    let role_name = request.role_name.clone();

    // Get target user's institution for validation
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
        role_name == "admin" && auth_user.user.institution_id == target_user.institution_id
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

/// Returns a vector of all routes defined in this module.
///
/// This function collects all the route handlers defined in this module
/// and returns them as a vector for registration with the Rocket framework.
///
/// # Returns
/// A vector containing all route handlers for user endpoints
pub fn routes() -> Vec<Route> {
    routes![create_user, list_users, get_user_roles_endpoint, add_user_role, remove_user_role]
}