// neems-core/src/api/institution.rs

use diesel::prelude::*;
use diesel::QueryableByName;
use diesel::sql_types::BigInt;
use rand::rng;
use rand::prelude::IndexedRandom;

use crate::models::{Institution, NewInstitution, InstitutionNoTime};

#[derive(QueryableByName)]
struct LastInsertRowId {
    #[diesel(sql_type = BigInt)]
    last_insert_rowid: i64,
}


pub fn random_energy_company_names(count: usize) -> Vec<&'static str> {
    let names = vec![
        "Voltara Energy", "Dynamis Power", "Nextera Solutions", "Energex Systems",
        "OmniVolt Energy", "PowerSphere", "TerraVolt", "Aegis Energy",
        "Lumina Power", "CoreGrid Energy", "StorVolt Technologies", "PowerCell Dynamics",
        "EverCharge Storage", "VoltCache", "NanoGrid Batteries", "EnerStore Solutions",
        "FluxCap Energy", "QuantumCell Storage", "PowerBank Energy", "IonCore Storage",
        "SunForge Energy", "HeliosRay Solar", "SolaraGrid", "Photonix Solar",
        "SunCore Renewables", "AuroraSolar Tech", "Lumos Solar", "Solaris Dynamics",
        "BrightVolt Solar", "SunTerra Power", "AeroVolt Wind", "GaleForce Energy",
        "WindStrider Power", "TurbineX Solutions", "Breeza Energy", "SkyBlade Wind",
        "Vortex Power", "ZephyrWind Tech", "Cyclone Energy", "WindHarbor Systems",
        "TitanGrid Power", "PrimeVolt Plants", "EcoGen Energy", "TerraFirma Power",
        "GridForge Solutions", "OmniPlant Energy", "NovaCore Power", "Apex Energy Plants",
        "VoltForge Utilities", "PowerHaven Energy", "AtomForge Energy", "Neutron Power Co.",
        "FissionX Solutions", "NucleoVolt", "QuantumFission Energy", "AtomGrid Systems",
        "CoreNova Power", "Isotope Dynamics", "ReactorX Energy", "Atomic Horizon",
        "Fusionis Energy", "StellarCore Fusion", "PlasmaVolt Tech", "Helion Dynamics",
        "SunFusion Power", "QuantumPlasma", "Ignis Fusion", "NovaFusion Systems",
        "ThermoCore Energy", "FusionForge", "HybridVolt Energy", "NextGen Power Co.",
        "SynthEnergy Solutions", "EcoVolt Innovations", "HyperGrid Energy", "GreenCore Dynamics",
        "Infinity Power", "OptiVolt Systems", "Aether Energy", "NeoVolt Technologies",
        "VerdePower", "EcoSphere Energy", "SustainaVolt", "GreenPulse Energy",
        "TerraWatt Solutions", "RenewaCore", "EarthVolt Power", "PureEnergy Dynamics",
        "BioVolt Renewables", "CleanGrid Tech", "QuantumVolt", "Hyperion Energy",
        "Nebula Power", "Astralis Energy", "CyberVolt", "NanoVolt Tech",
        "Pulsar Energy", "Omega Power Systems", "Xenon Energy", "Zenith Power Co.",
    ];
    let mut rng = rng();
    let selected: Vec<_> = names.choose_multiple(&mut rng, count).copied().collect();
    selected
}


/// Try to find an institution by name (case-sensitive).
/// Returns Ok(Some(Institution)) if found, Ok(None) if not, Err on DB error.
pub fn get_institution_by_name(
    conn: &mut SqliteConnection,
    inst: &InstitutionNoTime,
) -> Result<Option<Institution>, diesel::result::Error> {
    use crate::schema::institutions::dsl::*;
    let result = institutions
        .filter(name.eq(&inst.name))
        .first::<Institution>(conn)
        .optional()?;
    Ok(result)
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



#[cfg(test)]
mod tests {
    use super::*;
    use crate::orm::setup_test_db; 

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
