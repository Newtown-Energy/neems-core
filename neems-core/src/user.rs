use rand::rng;
use rand::prelude::IndexedRandom;
use rocket::http::{ContentType, Status};
use rocket::local::asynchronous::Client;
use rocket::response::status;
use rocket::Route;
use rocket::serde::json::{json, Json};
use rocket::tokio;

use crate::orm::DbConn;
use crate::orm::user::{insert_user, list_all_users};
use crate::models::{User, UserNoTime};

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

/// Helper to create a user via the API and return the created User
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



#[post("/1/users", data = "<new_user>")]
pub async fn create_user(
    db: DbConn,
    new_user: Json<UserNoTime>
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


#[get("/1/users")]
pub async fn list_users(
    db: DbConn
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

pub fn routes() -> Vec<Route> {
    routes![create_user, list_users]
}



#[tokio::test]
async fn test_admin_user_is_created() {
    use crate::orm::test_rocket;
    use rocket::local::asynchronous::Client;

    // Start Rocket with the admin fairing attached
    let rocket = test_rocket();
    let client = Client::tracked(rocket).await.expect("valid rocket instance");

    // Get a DB connection from the pool
    let conn = crate::orm::DbConn::get_one(client.rocket()).await
        .expect("get db connection");

    // Use the default admin email (from env or fallback)
    let admin_email = std::env::var("NEEMS_DEFAULT_USER").unwrap_or_else(|_| "admin@example.com".to_string());

    // Query for the admin user and verify it has the newtown-admin role
    let (found_user, has_admin_role) = conn.run(move |c| {
        use diesel::prelude::*;
        use crate::models::{User, Role};
        use crate::schema::users::dsl::*;
        use crate::schema::roles;
        use crate::schema::user_roles;

        // Find the admin user
        let user = users.filter(email.eq(admin_email))
            .first::<User>(c)
            .optional()
            .expect("user query should not fail");

        let has_role = if let Some(ref u) = user {
            // Check if the user has the newtown-admin role
            let role_exists = user_roles::table
                .inner_join(roles::table)
                .filter(user_roles::user_id.eq(u.id.expect("user should have id")))
                .filter(roles::name.eq("newtown-admin"))
                .first::<(crate::models::UserRole, Role)>(c)
                .optional()
                .expect("role query should not fail");
            
            role_exists.is_some()
        } else {
            false
        };

        (user, has_role)
    }).await;

    assert!(found_user.is_some(), "Admin user should exist after fairing runs");
    assert!(has_admin_role, "Admin user should have the newtown-admin role");
}
