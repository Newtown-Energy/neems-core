//! Demo-mode data bootstrap.
//!
//! A demo deployment starts with an empty database: `admin_init_fairing`
//! creates the company and admin user, and nothing else exists. Alarms work
//! regardless — definitions are compiled in and `alarm_state` is keyed on
//! `alarm_num` alone — but everything addressed *per site* (the SLD, E-stop,
//! SoC history, schedules) has nothing to point at until a site row exists.
//!
//! So when demo mode is on, ensure there is one. Deliberately minimal: one
//! site, created only when the deployment has none, so this can never
//! accumulate sites across restarts or paper over a real deployment's data.

use diesel::prelude::*;
use rocket::fairing::AdHoc;

use crate::{
    admin_init_fairing::find_company,
    api::demo::DemoMode,
    models::Site,
    orm::{
        DbConn,
        site::{get_all_sites, insert_site},
    },
};

/// Name of the seeded site. Intentionally generic: this repository is public,
/// so nothing here names a real customer, vendor or location.
const DEMO_SITE_NAME: &str = "Demo Site";
const DEMO_SITE_ADDRESS: &str = "New York, NY";
/// Manhattan, so map/SLD views have a plausible location without implying a
/// particular installation.
const DEMO_SITE_LATITUDE: f64 = 40.7128;
const DEMO_SITE_LONGITUDE: f64 = -74.0060;
/// Five minutes. An unremarkable ramp for a demo; nothing depends on the value.
const DEMO_SITE_RAMP_SECONDS: i32 = 300;

/// Create the demo site when demo mode is on and no site exists.
///
/// Attached after [`crate::admin_init_fairing`], which this depends on for the
/// company. A failure here logs and lets startup continue: demo data is a
/// convenience, and taking the whole deployment down because a demo site could
/// not be created would be a worse outcome than a demo with no site.
pub fn demo_seed_fairing() -> AdHoc {
    AdHoc::on_ignite("Demo Data Seed", |rocket| async {
        if !DemoMode::resolve(rocket.figment()).enabled() {
            return rocket;
        }

        let Some(conn) = DbConn::get_one(&rocket).await else {
            error!("[demo-seed] Could not get a DB connection; skipping demo seed.");
            return rocket;
        };

        match conn.run(seed_demo_site).await {
            Ok(Some(site)) => {
                info!("[demo-seed] Created demo site '{}' (id {}).", site.name, site.id)
            }
            Ok(None) => info!("[demo-seed] Sites already present; nothing to seed."),
            Err(e) => error!("[demo-seed] Demo site creation failed: {:?}", e),
        }

        rocket
    })
}

/// Ensure a site exists. Returns the created site, or `None` when one was
/// already present (or the company is missing, which means `admin_init` has not
/// run and there is nothing to attach a site to).
///
/// Keyed on "the deployment has no sites at all" rather than on the demo site's
/// name: the point is to give an empty demo something to address, and a
/// deployment that already has sites — seeded or real — does not need one.
fn seed_demo_site(conn: &mut SqliteConnection) -> Result<Option<Site>, diesel::result::Error> {
    if !get_all_sites(conn)?.is_empty() {
        return Ok(None);
    }

    // Resolve through admin_init's own lookup rather than a name of our own.
    // It accepts several historical spellings and only creates the canonical
    // one, so a deployment carrying "Newtown Energy, Inc." would never match a
    // hard-coded name here — and the demo would silently get no site.
    let company = match find_company(conn)? {
        Some(company) => company,
        None => {
            error!("[demo-seed] No Newtown Energy company found; skipping demo site.");
            return Ok(None);
        }
    };

    let site = insert_site(
        conn,
        DEMO_SITE_NAME.to_string(),
        DEMO_SITE_ADDRESS.to_string(),
        DEMO_SITE_LATITUDE,
        DEMO_SITE_LONGITUDE,
        company.id,
        DEMO_SITE_RAMP_SECONDS,
        None,
    )?;

    Ok(Some(site))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orm::testing::setup_test_db;

    // `setup_test_db` runs the migrations, one of which seeds the
    // 'Newtown Energy' company — so the fixture already has what the seed
    // needs, and starts with no sites.

    fn demo_company(conn: &mut SqliteConnection) -> crate::models::Company {
        find_company(conn)
            .expect("company query")
            .expect("migrations seed the demo company")
    }

    #[test]
    fn creates_a_site_when_the_deployment_has_none() {
        let mut conn = setup_test_db();

        let created = seed_demo_site(&mut conn).expect("seed").expect("a site");
        assert_eq!(created.name, DEMO_SITE_NAME);
        assert_eq!(get_all_sites(&mut conn).expect("sites").len(), 1);
    }

    /// Restarts must not accumulate sites — the fairing runs on every ignite.
    #[test]
    fn is_idempotent_across_restarts() {
        let mut conn = setup_test_db();

        seed_demo_site(&mut conn).expect("first seed");
        assert!(seed_demo_site(&mut conn).expect("second seed").is_none());
        assert_eq!(get_all_sites(&mut conn).expect("sites").len(), 1);
    }

    /// A deployment that already has sites is left alone, so this can never
    /// add a stray demo site to somewhere with real data.
    #[test]
    fn leaves_an_existing_site_alone() {
        let mut conn = setup_test_db();
        let company = demo_company(&mut conn);
        insert_site(
            &mut conn,
            "Existing Site".to_string(),
            "Somewhere".to_string(),
            1.0,
            2.0,
            company.id,
            60,
            None,
        )
        .expect("insert existing site");

        assert!(seed_demo_site(&mut conn).expect("seed").is_none());
        let sites = get_all_sites(&mut conn).expect("sites");
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].name, "Existing Site");
    }

    /// A deployment carrying one of the older company spellings must still get
    /// its demo site.
    ///
    /// `admin_init` matches several forms and only creates the canonical one,
    /// so a deployment named "Newtown Energy, Inc." never has a company called
    /// exactly "Newtown Energy" — and seeding against a hard-coded name would
    /// silently do nothing.
    #[test]
    fn finds_the_company_under_an_older_spelling() {
        use crate::{admin_init_fairing::COMPANY_CANDIDATE_NAMES, schema::companies};

        let mut conn = setup_test_db();
        let company = demo_company(&mut conn);
        let variant = COMPANY_CANDIDATE_NAMES[2];
        diesel::update(companies::table.find(company.id))
            .set(companies::name.eq(variant))
            .execute(&mut conn)
            .expect("rename the company to an older spelling");

        let created = seed_demo_site(&mut conn).expect("seed").expect("a site");
        assert_eq!(created.company_id, company.id);
        assert_eq!(get_all_sites(&mut conn).expect("sites").len(), 1);
    }

    /// Without the company there is nothing to attach to; report it rather than
    /// inventing one, since admin_init owns that decision.
    #[test]
    fn skips_when_the_company_is_missing() {
        let mut conn = setup_test_db();
        let company = demo_company(&mut conn);
        diesel::delete(crate::schema::companies::table.find(company.id))
            .execute(&mut conn)
            .expect("remove the seeded company");

        assert!(seed_demo_site(&mut conn).expect("seed").is_none());
        assert!(get_all_sites(&mut conn).expect("sites").is_empty());
    }
}
