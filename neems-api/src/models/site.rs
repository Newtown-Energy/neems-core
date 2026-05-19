use diesel::{Associations, Identifiable, Insertable, Queryable, QueryableByName, Selectable};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::schema::sites;

/// Variant flag for sites that need different demo behavior.
///
/// `Standard` is the typical interconnect. `NoGridCharge` represents the
/// alternate-site arc from the demo script — the inverters cannot pull
/// from the grid, so any charge command at this site is invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum SiteVariant {
    Standard,
    NoGridCharge,
}

impl SiteVariant {
    pub fn as_str(&self) -> &'static str {
        match self {
            SiteVariant::Standard => "standard",
            SiteVariant::NoGridCharge => "no_grid_charge",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "standard" => Ok(SiteVariant::Standard),
            "no_grid_charge" => Ok(SiteVariant::NoGridCharge),
            _ => Err(format!("Unknown site variant: {}", s)),
        }
    }
}

#[derive(
    Queryable,
    Selectable,
    Identifiable,
    Associations,
    QueryableByName,
    Clone,
    Debug,
    Serialize,
    Deserialize,
    TS,
)]
#[diesel(belongs_to(crate::models::company::Company))]
#[diesel(table_name = sites)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[ts(export)]
pub struct Site {
    pub id: i32,
    pub name: String,
    pub address: String,
    pub latitude: f64,
    pub longitude: f64,
    pub company_id: i32,            // Foreign key to Company
    pub ramp_duration_seconds: i32, // Time to ramp from 0 to full power (default 120s)
    pub power_kw: Option<f64>,
    pub capacity_kwh: Option<f64>,
    pub closed_loop_enabled: bool,
    pub off_peak_start_minutes: Option<i32>,
    pub off_peak_end_minutes: Option<i32>,
    pub peak_revenue_start_minutes: Option<i32>,
    pub peak_revenue_end_minutes: Option<i32>,
    pub interconnection_max_output_kw: Option<f64>,
    pub rebound_protection_soc_floor_percent: f64,
    pub site_variant: String,
    /// Charge ceiling as a percentage of `power_kw` (0–100). Drives the
    /// height of the orange charge bar in the calendar day cell so a
    /// site that charges at half-power renders a half-height bar.
    pub charge_rate_percent: f64,
    /// Discharge ceiling as a percentage of `power_kw` (0–100). See
    /// `charge_rate_percent` for visualization context.
    pub discharge_rate_percent: f64,
}

#[derive(Insertable)]
#[diesel(table_name = sites)]
pub struct NewSite {
    pub name: String,
    pub address: String,
    pub latitude: f64,
    pub longitude: f64,
    pub company_id: i32,
    pub ramp_duration_seconds: i32,
    // Optional demo-driven defaults. The ORM `insert_site` populates
    // these so newly-created sites start with sane scheduling values
    // instead of nulls (see `DEFAULT_*` constants in `orm::site`).
    pub power_kw: Option<f64>,
    pub capacity_kwh: Option<f64>,
    pub off_peak_start_minutes: Option<i32>,
    pub off_peak_end_minutes: Option<i32>,
    pub peak_revenue_start_minutes: Option<i32>,
    pub peak_revenue_end_minutes: Option<i32>,
    pub interconnection_max_output_kw: Option<f64>,
}

// For API inputs and validation
#[derive(Deserialize, Serialize, TS)]
#[ts(export)]
pub struct SiteInput {
    pub name: String,
    pub address: String,
    pub latitude: f64,
    pub longitude: f64,
    pub company_id: i32,
    pub ramp_duration_seconds: i32,
}

// Response struct that includes computed timestamps from activity log
#[derive(Debug, Serialize, TS)]
#[ts(export)]
pub struct SiteWithTimestamps {
    pub id: i32,
    pub name: String,
    pub address: String,
    pub latitude: f64,
    pub longitude: f64,
    pub company_id: i32,
    pub ramp_duration_seconds: i32,
    pub power_kw: Option<f64>,
    pub capacity_kwh: Option<f64>,
    pub closed_loop_enabled: bool,
    pub off_peak_start_minutes: Option<i32>,
    pub off_peak_end_minutes: Option<i32>,
    pub peak_revenue_start_minutes: Option<i32>,
    pub peak_revenue_end_minutes: Option<i32>,
    pub interconnection_max_output_kw: Option<f64>,
    pub rebound_protection_soc_floor_percent: f64,
    pub site_variant: String,
    pub charge_rate_percent: f64,
    pub discharge_rate_percent: f64,
    #[ts(type = "string")]
    pub created_at: chrono::NaiveDateTime,
    #[ts(type = "string")]
    pub updated_at: chrono::NaiveDateTime,
}
