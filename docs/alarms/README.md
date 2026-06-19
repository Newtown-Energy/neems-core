# Newtown Alarm & SLD Spec

Structured capture of the **client alarm/SLD source spreadsheet** (sheets
`Digitals` and `Analogs`). This is the intermediate artifact for replacing
alarms and data points across all three components:

> **Keep the source spreadsheet out of the repository.** Point the generator at
> it via the `ALARM_XLSX` env var (see below); only the derived JSON is
> committed. The workbook itself — its filename plus any sheets, columns, or
> notes beyond what this generator extracts — may carry site-specific detail we
> don't want to publish, and this repo is public.
>
> **The derived JSON is _not_ anonymized.** Alarm and equipment names (Tesla
> Megapack, SEL relays, etc.) are copied verbatim from the spreadsheet — the
> same names already used in `neems-data/src/rtac/alarm_definitions.rs`. The
> generator does no redaction of its own. If a particular name must stay
> private, remove it in the source spreadsheet before regenerating; that keeps
> the private string out of this (public) repo entirely, including out of the
> generator code.

- **neems-core** (Rust backend) — `neems-data/src/rtac/alarm_definitions.rs`,
  `neems-data/src/rtac/protocol.rs`
- **neems-react** (frontend) — SLD components, `siteConfig.ts`, generated types
- **neems-rtac-sim** (Modbus simulation) — reuses the `rtac::protocol` register map

## Files

| File | Purpose |
|------|---------|
| `newtown-alarms.json` | The generated spec (do not hand-edit; regenerate instead). |
| `build_alarm_spec.py` | Generator (needs `openpyxl`). See "How to regenerate" below. |
| _(source `.xlsx`)_ | The client spreadsheet — kept **outside** the repo, never committed. |

Regenerating is deterministic — same spreadsheet in, same JSON out (no
timestamps/randomness), so the file diffs cleanly when the client sends a new
version.

## Top-level shape of `newtown-alarms.json`

```
metadata                  – source info, conventions, counts
reference                 – lookup tables (see below)
megapack_digital_template – canonical 30-bit-per-MP status template (from MP-1A)
megapack_analog_template  – canonical 30-measurement-per-MP template (from MP-1A)
digital_alarms[]          – one entry per Digitals row (status bits)
analog_points[]           – one entry per Analogs row (measurements)
data_quality_issues[]     – detected spreadsheet anomalies, see below
```

### Key convention: `alarm_num` is namespaced by category

`alarm_num` is unique **within** a category but **not** across them. `601` is
both a digital MP-1A status bit (`megapack_loss_of_comms`) **and** an analog
MP-1A measurement (`real_power_target`). Always key by **(category, alarm_num)**.

### Key convention: source vs. derived fields

- Values taken verbatim from the spreadsheet use plain or `*_raw` keys
  (`name`, `zone_raw`, `sld.change_raw`, `threshold_raw`, `mouseover`, …).
- Anything we computed is grouped and tagged `"_derived": true`
  (`suggested_code_name`, `modbus`, `severity_signals`, `threshold`,
  `zone_inferred`). Treat derived fields as a starting point, not gospel —
  especially `suggested_code_name` (a mechanical slug) and `severity_signals`.

## `digital_alarms[]` entry

```jsonc
{
  "alarm_num": 3,
  "category": "digital",
  "zone_raw": "Newtown",            // spreadsheet "Alarm Zone"
  "zone": "Site",                   // canonical AlarmZone (matches Rust enum)
  "zone_inferred": false,           // true => zone backfilled from numbering block
  "sld_component_id": "site",       // React SLD component id (ZONE_TO_COMPONENT)
  "name": "No IP connection to site",
  "suggested_code_name": "no_ip_connection_to_site",  // _derived slug
  "reserved": false,                // blank / "[RESERVED]" / "...Reserved" rows
  "pt_number": null,                // spreadsheet "Pt number (DNP3 or Modbus)"
  "sld": {
    "related_objects_raw": "Net, Border",
    "related_objects": ["Net", "Border"],   // split on comma
    "change_raw": "Main obj: Red, flashing; Border obj: blue, flash",
    "changes": [                            // best-effort split on ";" then "target:"
      { "target": "Main obj",   "instruction": "Red, flashing" },
      { "target": "Border obj", "instruction": "blue, flash" }
    ]
  },
  "availability_impact": "site_offline",    // null | "site_offline" | "mp_offline"
  "mouseover": "Network communications: ...",  // surrounding quotes stripped
  "is_fire": false,                          // spreadsheet "IsFire?" == Y
  "severity_signals": {                      // _derived — raw inputs for level mapping
    "primary_color": "red",                  // color of the Main obj
    "flashing": true,
    "is_fire": false,
    "availability_impact": "site_offline"
  },
  "modbus": {                                // _derived from the register layout
    "register_index": 0,                     // 0-based within the 22-register alarm block
    "register_address": 8,                   // holding-register address = 8 + register_index
    "bit": 2                                 // bit position 0-15
  }
}
```

`modbus` is `null` when the alarm number falls outside its zone's register
allocation (only a few stray trailing reserved numbers do — see issues).

## `analog_points[]` entry

```jsonc
{
  "alarm_num": 619,
  "category": "analog",
  "zone_raw": "MP-1A",
  "zone": "Mp1a",
  "sld_component_id": "megapack-1a",
  "name": "max_battery_temperature",
  "suggested_code_name": "max_battery_temperature",
  "spare": false,                       // AI_spare_* / DI_spare_* points
  "pt_number": null,                    // analogs are unnumbered in the spreadsheet
  "sld": { "related_objects_raw": "MP", "related_objects": ["MP"], "change_raw": null, "changes": [] },
  "availability_impact": null,
  "threshold_raw": "60C",
  "threshold": { "value": 60, "unit": "C" },   // _derived parse
  "mouseover": null,
  "is_fire": true,
  "alarm_levels": null                   // spreadsheet "Alarm Levels" col (unused so far)
}
```

> **Note:** Analog points have **no Modbus register assignment** in the
> spreadsheet. The 30-per-MP measurement block maps to the Tesla analog input
> registers, but the numbering is TBD and must be sourced separately before the
> simulation can emit analogs. `megapack_analog_template[].offset` (0–29)
> preserves the spreadsheet order as the only ordering hint we have.

## `reference` section

- **`zones`** — every spreadsheet zone label (digital `MP1A_digital` and analog
  `MP-1A` both included) → canonical `zone` → React `sld_component_id`.
- **`severity_levels`** — the 1–5 Newtown alarm matrix, for mapping (see below).
- **`availability_impacts`** — `site_offline`, `mp_offline`.
- **`sld_change_color_legend`** — what red/yellow/green/blue and border colors mean.
- **`sld_object_tokens`** — every distinct token used in the "Related SLD Object"
  columns with occurrence counts (useful for reconciling against the React SLD
  element ids, which differ — e.g. spreadsheet `52-MAIN-1` vs component
  `breaker-main`, `MP` vs `megapack-1a`).

## Severity: how to assign levels (deliberately not baked in)

The spreadsheet does **not** carry an explicit 1–5 level per row. It encodes
severity indirectly, captured under `severity_signals`. Recommended mapping for
the implementation step (mirrors the existing `alarm_definitions.rs` intent):

| Signal | Level |
|--------|-------|
| `is_fire` true **and** mouseover "FIRE!!" / suppression / FLIR | **1 – Emergency** |
| `primary_color` red **and** `availability_impact` = `site_offline`/`mp_offline` | **2 – High** |
| `primary_color` red, no availability impact | **3 – Medium** |
| `primary_color` yellow | **4 – Low** |
| reserved/spare, no color, informational | **5 – Info** |

This is a starting heuristic — the existing Rust `ALARM_DEFINITIONS` has
hand-tuned per-alarm levels that should be reconciled against it rather than
blindly overwritten.

## `data_quality_issues[]`

Genuine anomalies detected during generation (8 at last run):

- **Breaker 133–135** — reserved numbers that overflow the 2-register
  (32-bit) breaker allocation; no Modbus bit assigned (`modbus: null`).
- **MP analog offset 14 (alarms 645/675/705/735/765)** — labeled
  `ac_voltage_phaseA` in blocks 1B–2C where the MP-1A template has
  `inverter_phaseA_current`. Almost certainly a spreadsheet copy/paste error;
  decide the correct name before generating analog definitions.

Other things noticed while parsing (not flagged programmatically, worth a human
eye during implementation):

- Alarm 102 mouseover reads `"89L12 Open"` — likely a typo for `89L2 Open`.
- The Analogs sheet has a duplicate, always-empty "MP or Site Availability"
  column (col 9); it is ignored.
- The `Outputs` and `SLD reference image` sheets carry no tabular data (the
  latter is just the reference diagram image).

## How to regenerate

```bash
# openpyxl required (pip install openpyxl). The source .xlsx is kept outside the
# repo; point the generator at it. If a single .xlsx sits in the project root
# (the dir containing neems-core) it is picked up automatically.
ALARM_XLSX=/path/to/source.xlsx python3 neems-core/docs/alarms/build_alarm_spec.py
```
