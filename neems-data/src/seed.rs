//! Generation of plausible historical readings (SoC + alarms) for demos.
//!
//! This lives in the library (rather than the `neems-data` binary) so it is
//! shared by both the `seed-soc-history` / `seed-alarm-history` CLI commands
//! and the neems-api demo endpoint — the two therefore produce byte-identical
//! data.
//!
//! Both seeders are idempotent: existing reading timestamps for the source are
//! collected up-front and only missing slots are written, so re-running only
//! fills gaps.

use std::{collections::HashSet, error::Error};

use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use diesel::prelude::*;
use serde_json::json;

use crate::{
    NewReading, NewSource,
    collectors::data_sources::charging_state_with_level,
    create_source, insert_readings_batch,
    rtac::state::AlarmFlags,
    schema::{readings, sources},
};

/// Summary of a single seed run.
#[derive(Debug, Clone)]
pub struct SeedOutcome {
    pub source_id: i32,
    pub source_name: String,
    /// New readings actually written this run.
    pub written: usize,
    /// Slots skipped because a reading already existed at that timestamp.
    pub already_present: usize,
    /// Total slots spanned by the window (written + already_present).
    pub total_slots: usize,
}

/// Deterministic demo alarm state for a given instant.
///
/// Each tuple is `(alarm_num, period_minutes, active_minutes, phase_minutes)`:
/// the alarm is active when the time-of-window position is within the first
/// `active_minutes` of each `period_minutes` cycle. The chosen alarms span
/// several zones and severities so the FDNY timeline has variety, and the
/// long periods keep transitions sparse (a handful per alarm per week) rather
/// than flapping every sample.
pub fn seeded_alarm_flags(utc: DateTime<Utc>) -> AlarmFlags {
    const PATTERN: &[(u16, i64, i64, i64)] = &[
        (1, 1440, 90, 0),       // loss_fiber (L3) — ~daily, 90 min
        (203, 2880, 180, 600),  // meter_loss_of_comms (L5) — every 2 days, 3 h
        (301, 720, 60, 200),    // t1_temp_alarm (L4) — twice daily, 1 h
        (104, 4320, 240, 1000), // estop (L2, critical) — every 3 days, 4 h
        (7, 5760, 30, 2500),    // intruder_detected (L5) — every 4 days, 30 min
    ];
    let t_min = utc.timestamp() / 60;
    let mut flags = AlarmFlags::default();
    for &(num, period, active, phase) in PATTERN {
        let pos = ((t_min - phase) % period + period) % period;
        if pos < active {
            flags.set_alarm_num(num, true);
        }
    }
    flags
}

/// Shared backfill loop: ensures a seed-only source exists for the site,
/// builds an epoch-aligned slot grid over the last `days`, and writes a
/// reading for each missing slot using `make_blob` to render the JSON data.
fn seed_history<F>(
    conn: &mut SqliteConnection,
    site_id: i32,
    days: u32,
    interval_minutes: u32,
    test_type: &str,
    name_prefix: &str,
    description_kind: &str,
    make_blob: F,
) -> Result<SeedOutcome, Box<dyn Error + Send + Sync>>
where
    F: Fn(i32, DateTime<Utc>) -> String,
{
    if interval_minutes == 0 {
        return Err("interval_minutes must be > 0".into());
    }

    // Ensure a source exists for this site/test_type. Reuse the existing one if
    // present; otherwise create a deterministic name so re-runs find the same
    // row.
    let existing_source: Option<(Option<i32>, String)> = sources::table
        .filter(sources::site_id.eq(site_id))
        .filter(sources::test_type.eq(test_type))
        .select((sources::id, sources::name))
        .first(conn)
        .optional()?;

    let (source_id, source_name) = match existing_source {
        Some((Some(id), name)) => (id, name),
        Some((None, name)) => {
            return Err(format!("source '{}' has NULL id (corrupt row?)", name).into());
        }
        None => {
            let name = format!("{}_site_{}", name_prefix, site_id);
            let new_source = NewSource {
                name: name.clone(),
                description: Some(format!(
                    "Demo {} for site {} (seeded)",
                    description_kind, site_id
                )),
                active: Some(false), // seed-only; not polled live
                interval_seconds: Some((interval_minutes as i32) * 60),
                test_type: Some(test_type.to_string()),
                arguments: Some("{}".to_string()),
                site_id: Some(site_id),
                company_id: None,
            };
            let created = create_source(conn, new_source)?;
            let id = created.id.ok_or("create_source returned a row with no id")?;
            (id, created.name)
        }
    };

    // Build the slot grid (oldest → newest, top of the minute), snapping the
    // end to the most recent slot boundary so re-runs hit the same timestamps.
    let interval = Duration::minutes(interval_minutes as i64);
    let end = {
        let secs = Utc::now().timestamp();
        let slot = interval.num_seconds();
        let snapped = secs - (secs % slot);
        DateTime::from_timestamp(snapped, 0)
            .ok_or("failed to snap end timestamp")?
            .naive_utc()
    };
    let start = end - Duration::days(days as i64);
    // Inclusive on both endpoints, so slot count is span/interval + 1.
    let total_slots = ((end - start).num_seconds() / interval.num_seconds()) as usize + 1;

    // Idempotency: pull existing timestamps in the window and skip them.
    let existing: HashSet<NaiveDateTime> = readings::table
        .filter(readings::source_id.eq(source_id))
        .filter(readings::timestamp.ge(start))
        .filter(readings::timestamp.le(end))
        .select(readings::timestamp)
        .load::<NaiveDateTime>(conn)?
        .into_iter()
        .collect();

    let mut batch: Vec<NewReading> = Vec::new();
    let mut cursor = start;
    while cursor <= end {
        if !existing.contains(&cursor) {
            batch.push(NewReading {
                source_id,
                timestamp: Some(cursor),
                data: make_blob(source_id, cursor.and_utc()),
                quality_flags: Some(0),
            });
        }
        cursor += interval;
    }

    let written = batch.len();
    // Insert in chunks so SQLite doesn't choke on a giant single statement.
    for chunk in batch.chunks(500) {
        insert_readings_batch(conn, chunk.to_vec())?;
    }

    Ok(SeedOutcome {
        source_id,
        source_name,
        written,
        already_present: total_slots - written,
        total_slots,
    })
}

/// Backfill plausible past SoC readings for the given site.
pub fn seed_soc_history(
    conn: &mut SqliteConnection,
    site_id: i32,
    days: u32,
    interval_minutes: u32,
) -> Result<SeedOutcome, Box<dyn Error + Send + Sync>> {
    seed_history(
        conn,
        site_id,
        days,
        interval_minutes,
        "charging_state",
        "soc_history",
        "SoC history",
        |source_id, utc| {
            let (state, level) = charging_state_with_level(utc, "default");
            json!({
                "source_id": source_id,
                "battery_id": "default",
                "state": state,
                "level": level,
                "timestamp_utc": utc.to_rfc3339(),
                "seeded": true,
            })
            .to_string()
        },
    )
}

/// Backfill plausible past alarm readings for the given site.
pub fn seed_alarm_history(
    conn: &mut SqliteConnection,
    site_id: i32,
    days: u32,
    interval_minutes: u32,
) -> Result<SeedOutcome, Box<dyn Error + Send + Sync>> {
    seed_history(
        conn,
        site_id,
        days,
        interval_minutes,
        "alarm_status",
        "alarm_history",
        "alarm history",
        |source_id, utc| {
            let registers = seeded_alarm_flags(utc).to_registers();
            json!({
                "source_id": source_id,
                "alarm_registers": registers.to_vec(),
                "timestamp_utc": utc.to_rfc3339(),
                "seeded": true,
            })
            .to_string()
        },
    )
}
