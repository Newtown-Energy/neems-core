// neems-core/src/api/institution.rs

use diesel::prelude::*;
use diesel::QueryableByName;
use diesel::sql_types::BigInt;
use rand::prelude::IndexedRandom;
use rocket::serde::json::Json;
use rocket::http::Status;
use rocket::response::status;
use rocket::Route;

use crate::db::DbConn;
use crate::models::{Institution, NewInstitution, InstitutionName};

#[derive(QueryableByName)]
struct LastInsertRowId {
    #[diesel(sql_type = BigInt)]
    last_insert_rowid: i64,
}

use rand::rng;

pub fn random_energy_company_names(count: usize) -> Vec<&'static str> {
    let names = vec![
        // General Energy & Power
        "Voltara Energy",
        "Dynamis Power",
        "Nextera Solutions",
        "Energex Systems",
        "OmniVolt Energy",
        "PowerSphere",
        "TerraVolt",
        "Aegis Energy",
        "Lumina Power",
        "CoreGrid Energy",
        // Battery & Energy Storage
        "StorVolt Technologies",
        "PowerCell Dynamics",
        "EverCharge Storage",
        "VoltCache",
        "NanoGrid Batteries",
        "EnerStore Solutions",
        "FluxCap Energy",
        "QuantumCell Storage",
        "PowerBank Energy",
        "IonCore Storage",
        // Solar Energy
        "SunForge Energy",
        "HeliosRay Solar",
        "SolaraGrid",
        "Photonix Solar",
        "SunCore Renewables",
        "AuroraSolar Tech",
        "Lumos Solar",
        "Solaris Dynamics",
        "BrightVolt Solar",
        "SunTerra Power",
        // Wind Energy
        "AeroVolt Wind",
        "GaleForce Energy",
        "WindStrider Power",
        "TurbineX Solutions",
        "Breeza Energy",
        "SkyBlade Wind",
        "Vortex Power",
        "ZephyrWind Tech",
        "Cyclone Energy",
        "WindHarbor Systems",
        // Power Plants
        "TitanGrid Power",
        "PrimeVolt Plants",
        "EcoGen Energy",
        "TerraFirma Power",
        "GridForge Solutions",
        "OmniPlant Energy",
        "NovaCore Power",
        "Apex Energy Plants",
        "VoltForge Utilities",
        "PowerHaven Energy",
        // Nuclear Energy
        "AtomForge Energy",
        "Neutron Power Co.",
        "FissionX Solutions",
        "NucleoVolt",
        "QuantumFission Energy",
        "AtomGrid Systems",
        "CoreNova Power",
        "Isotope Dynamics",
        "ReactorX Energy",
        "Atomic Horizon",
        // Fusion Energy
        "Fusionis Energy",
        "StellarCore Fusion",
        "PlasmaVolt Tech",
        "Helion Dynamics",
        "SunFusion Power",
        "QuantumPlasma",
        "Ignis Fusion",
        "NovaFusion Systems",
        "ThermoCore Energy",
        "FusionForge",
        // Hybrid & Advanced Energy
        "HybridVolt Energy",
        "NextGen Power Co.",
        "SynthEnergy Solutions",
        "EcoVolt Innovations",
        "HyperGrid Energy",
        "GreenCore Dynamics",
        "Infinity Power",
        "OptiVolt Systems",
        "Aether Energy",
        "NeoVolt Technologies",
        // Sustainable & Green Energy
        "VerdePower",
        "EcoSphere Energy",
        "SustainaVolt",
        "GreenPulse Energy",
        "TerraWatt Solutions",
        "RenewaCore",
        "EarthVolt Power",
        "PureEnergy Dynamics",
        "BioVolt Renewables",
        "CleanGrid Tech",
        // Futuristic & High-Tech Energy
        "QuantumVolt",
        "Hyperion Energy",
        "Nebula Power",
        "Astralis Energy",
        "CyberVolt",
        "NanoVolt Tech",
        "Pulsar Energy",
        "Omega Power Systems",
        "Xenon Energy",
        "Zenith Power Co.",
    ];

    let mut rng = rng();
    let selected: Vec<_> = names.choose_multiple(&mut rng, count).copied().collect();
    selected
}

pub fn insert_institution(
    conn: &mut SqliteConnection, 
    inst_name: String,
) -> Result<Institution, diesel::result::Error> {
    use crate::schema::institutions::dsl::*;
    let now = chrono::Utc::now().naive_utc();

    let new_inst = NewInstitution {
        name: inst_name,
        created_at: Some(now),
        updated_at: Some(now),
    };

    diesel::insert_into(institutions)
        .values(&new_inst)
        .execute(conn)?;

    let last_id = diesel::sql_query("SELECT last_insert_rowid() as last_insert_rowid")
        .get_result::<LastInsertRowId>(conn)?
        .last_insert_rowid;

    institutions
        .filter(id.eq(last_id as i32))
        .first::<Institution>(conn)
}


#[post("/institutions", data = "<new_institution>")]
pub async fn create_institution(
    db: DbConn,
    new_institution: Json<InstitutionName>
) -> Result<status::Created<Json<Institution>>, Status> {
    db.run(move |conn| {
        insert_institution(conn, new_institution.name.clone())
            .map(|inst| status::Created::new("/").body(Json(inst)))
            .map_err(|_| Status::InternalServerError)
    }).await
}

#[get("/institutions")]
pub async fn list_institutions(
    db: DbConn
) -> Result<Json<Vec<Institution>>, Status> {
    db.run(|conn| {
        use crate::schema::institutions::dsl::*;
        institutions
            .order(id.asc())
            .load::<Institution>(conn)
            .map(Json)
            .map_err(|_| Status::InternalServerError)
    }).await
}

// Helper to return all routes for mounting
pub fn routes() -> Vec<Route> {
    routes![create_institution, list_institutions]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::setup_test_db; 

    #[test]
    fn test_insert_institution() {
	let mut conn = setup_test_db();
	let result = insert_institution(&mut conn, "Test Institution".to_string());
	assert!(result.is_ok());
	let inst = result.unwrap();
	assert_eq!(inst.name, "Test Institution");

	let now = chrono::Utc::now().naive_utc();
	let diff_created = (inst.created_at - now).num_seconds().abs();
	let diff_updated = (inst.updated_at - now).num_seconds().abs();

	assert!(
	    diff_created <= 1,
	    "created_at should be within 1 second of now (diff: {})",
	    diff_created
	);
	assert!(
	    diff_updated <= 1,
	    "updated_at should be within 1 second of now (diff: {})",
	    diff_updated
	);
    }
}
