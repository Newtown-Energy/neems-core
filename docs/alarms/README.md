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
> **The derived JSON is only redacted to the extent you tell it to be.** Alarm
> and equipment names (Tesla Megapack, SEL relays, etc.) are copied verbatim
> from the spreadsheet — the same names already used in
> `neems-data/src/rtac/alarm_definitions.rs`. Anything that must stay private
> goes in **`alarm-redactions.tsv`**, a tab-separated `find<TAB>replace` file
> kept beside the workbook, **outside the repo** (override the path with
> `ALARM_REDACTIONS`). The generator applies it as cells are read, so slugs,
> templates and token tables are all clean by construction. A blank replacement
> deletes the text.
>
> The strings live outside the repo because they are exactly what must not be
> published — listing them in the generator would defeat the point. **If the
> file is missing, the generator says so and emits the workbook verbatim**;
> that line is the one to watch for on every regeneration. The workbook is a
> living document and the client can add a vendor or site name to any cell at
> any time — this happened on 2026-08-14 and was caught only by reading the
> diff, which is not a control. Sweep with
> `grep -rinE '<vendor>|<site>' .` before committing regardless.

- **neems-core** (Rust backend) — `neems-data/src/rtac/alarm_definitions.rs`,
  `neems-data/src/rtac/protocol.rs`
- **neems-react** (frontend) — SLD components, `siteConfig.ts`, generated types
- **neems-rtac-sim** (Modbus simulation) — reuses the `rtac::protocol` register map

## Files

| File | Purpose |
|------|---------|
| `newtown-alarms.json` | The generated spec (do not hand-edit; regenerate instead). |
| `build_alarm_spec.py` | Spreadsheet → JSON generator (needs `openpyxl`). See "How to regenerate" below. |
| `build_alarm_meta_rs.py` | JSON → Rust generator for `neems-data/src/rtac/alarm_sld_meta.rs`. |
| `build_analog_points_rs.py` | JSON → Rust generator for `neems-data/src/rtac/analog_points.rs` (the analog register map). |
| _(source `.xlsx`)_ | The client spreadsheet — kept **outside** the repo, never committed. |
| _(`alarm-redactions.tsv`)_ | Private strings to scrub, `find<TAB>replace` per line — kept **outside** the repo beside the workbook. |

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

### Key convention: `alarm_num` is the point's Modbus address

The client's alarm number **is** the point's Modbus address on the RTAC:

| Category | Modbus space | Read with | Address |
|----------|--------------|-----------|---------|
| analog   | registers    | FC 3 / 4  | `alarm_num` |
| digital  | discrete inputs | FC 2   | `alarm_num` |

`alarm_num` is therefore unique **within** a category but **not** across them —
`601` is both a digital MP-1A status bit (`megapack_loss_of_comms`) and an
analog MP-1A measurement (`real_power_target`). Always key by **(category,
alarm_num)**. The two never collide on the wire, because Modbus addresses bits
and registers in separate spaces.

Two assumptions sit behind this, both isolated to one place in the backend so
they are cheap to correct:

- **Numbering base.** We read the spreadsheet numbers literally — 601 is wire
  address 601 — rather than treating them as 1-based point numbers. Held in
  `RegisterMap::POINT_NUMBER_BASE`. If readings come back one point out, that
  is the line to change. **The client's "analogs are floats" now puts the
  stronger form of this assumption in doubt** — see "Float width" below; a
  32-bit float would make the numbers indexes rather than addresses, and the
  correction is a stride, not an offset.
- **Which register table.** Analogs are read as holding registers (FC 3) for
  now; input registers (FC 4) would be the more literal fit for read-only
  measurements, and the client has since confirmed both sheets are read-only.
  That strengthens the case for FC 4 (and FC 2 for the digital bits) without
  settling it: RTACs commonly expose the same data in both spaces. Nothing else
  depends on the choice — the backend's own command registers were moved to
  1000-1004 so they clear the client's whole 101-102 / 601-780 range either
  way.

### Key convention: source vs. derived fields

- Values taken verbatim from the spreadsheet use plain or `*_raw` keys
  (`name`, `zone_raw`, `sld.change_raw`, `threshold_raw`, `mouseover`, …).
- Anything we computed is grouped and tagged `"_derived": true`
  (`suggested_code_name`, `modbus`, `severity_signals`, `threshold`,
  `zone_inferred`). Treat derived fields as a starting point, not gospel —
  especially `suggested_code_name` (a mechanical slug) and `severity_signals`.
- `value_type` and `access` are a third class: **stated by the client, carried
  by neither the cells nor a computation.** No column says a Digitals row is a
  bit or an Analogs row a real number, and nothing marks a point writable. They
  are set from `CATEGORY_VALUE_TYPE` / `CATEGORY_ACCESS` in
  `build_alarm_spec.py`, so the client's answer lives in the spec rather than in
  a conversation. Every point either sheet defines is `read_only`; writable
  points are to arrive on a sheet that does not exist yet, which is why
  `access` is per entry rather than one flag. What those points will be, and
  the request lifecycle an operator's click travels through to reach one, is
  specified in [`../site-inputs.md`](../site-inputs.md).

## `digital_alarms[]` entry

```jsonc
{
  "alarm_num": 3,
  "category": "digital",
  "value_type": "bool",             // client-stated: every Digitals row is a bit
  "access": "read_only",            // client-stated: writable points get a future sheet
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
  "modbus": {
    "discrete_address": 3,                   // the client's address: == alarm_num
    "register_index": 0,                     // 0-based within the 22-register alarm block
    "register_address": 8,                   // holding-register address = 8 + register_index
    "bit": 2                                 // bit position 0-15
  }
}
```

`discrete_address` is the **client's** addressing — the alarm number itself, in
the discrete-input space. The other three describe something different: the
packed 22-register holding block the backend and simulator currently use, which
is **our own framing** and predates knowing how the RTAC addresses these points.
The analog side has moved onto the client's numbering; the digital read path has
not yet, because that touches the E-stop and alarm paths.

`register_index`/`register_address`/`bit` are `null` when the alarm number falls
outside its zone's register allocation (only a few stray trailing reserved
numbers do — see issues). `discrete_address` is always present.

## `analog_points[]` entry

```jsonc
{
  "alarm_num": 619,
  "category": "analog",
  "value_type": "float",            // client-stated; see "Float width" below
  "access": "read_only",
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
  "alarm_levels": null,                  // spreadsheet "Alarm Levels" col (unused so far)
  "modbus": { "register_address": 619 }  // == alarm_num
}
```

The address needs no computing: point 619 is register 619. Each Megapack owns a
contiguous 30-register block (MP-1A 601–630 … MP-2C 751–780), and
`megapack_analog_template[].offset` (0–29) is the position within one — so
`max_battery_temperature` at offset 18 is point 601 + 18 = 619 on MP-1A. The
only analogs outside a pack block are the two transformer winding temperatures
at 101 and 102.

> **Units and scaling are still unknown.** The spreadsheet gives neither for
> any analog row. The backend decodes only the three points the SLD renders
> (`state_of_energy`, `ac_voltage`, `max_battery_temperature`), using the
> encoding the site-level status registers already use — an assumption, not a
> specification, documented at `MegapackAnalogs` in `protocol.rs`. Every other
> point is carried through as a raw register value rather than given an
> invented encoding. A right address with a wrong scale reports 8.25% where the
> pack means 82.5%, silently and plausibly.

> **Float width is unsettled, and it moves the addresses.** The client says
> every Analogs row is a float, but the sheet numbers analog points one apart
> and a 32-bit float needs two Modbus registers. Both cannot describe the wire,
> so exactly one of these holds:
>
> - **16-bit scaled integers**, and "float" describes the engineering value.
>   The current model is right in kind; `MP_ANALOG_ENCODING` in `protocol.rs`
>   keeps its shape and only its divisors remain unconfirmed.
> - **True 32-bit floats**, in which case the spreadsheet numbers are point
>   *indexes*, not register addresses — the address becomes `base + 2*offset`,
>   the block read grows from 30 registers to 60, `AnalogEncoding` loses
>   `divisor`/`signed` for `f32::from_bits`, and **word order** becomes a new
>   thing to confirm.
>
> The generator flags this in `data_quality_issues[]` on every run until the
> client answers, and retires the flag by itself if the numbering ever changes.
> Note that floats would remove the wrong-divisor risk but not the units
> question: kW vs W and C vs F are still unspecified either way.

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

Genuine anomalies detected during generation (9 at last run):

- **Analog float width vs. numbering** — analogs are declared float but
  numbered one apart, which a 32-bit float cannot fit. Not a spreadsheet
  mistake but an unresolved question about the wire; see "Float width" above.
  The only issue in the list with a null `alarm_num`, because it is about the
  set rather than a row.
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
  latter is just the reference diagram image). `Outputs` is where the writable
  points are expected to land — the nine controls waiting on it are enumerated
  in [`../site-inputs.md`](../site-inputs.md), so a populated sheet can be
  checked against what the SLD already offers. (The E-stop is not among them:
  it goes out through our own command registers and needs nothing from this
  sheet.)

## How to regenerate

The data flows in two steps — **run all three** after the client sends a new
workbook: spreadsheet → `newtown-alarms.json` → the generated Rust tables
(`neems-data/src/rtac/alarm_sld_meta.rs` and `analog_points.rs`). Updating only
the JSON leaves the API serving stale messages/targets and the analog register
map pointing at the old addresses.

**Read `data_quality_issues[]` before committing a regeneration.** It flags
duplicate point numbers, which — since the point number became the Modbus
address — mean an ambiguous address rather than a bookkeeping slip. Check the
diff for new vendor or site names too, and confirm the run reported applying
redactions rather than "no redactions file found" (see the note at the top).

### Corrections applied to the workbook as read

The client's workbook carries mistakes we fix in `build_alarm_spec.py` rather
than by editing their file, so a regeneration cannot silently undo them:

- **`DIGITAL_RENUMBER`** — rows numbered wrongly at source. Currently one:
  `ANSI function PSV05T`, appended below the reserved block on 2026-08-14 and
  numbered 126, which `PSV04T` already holds. Renumbered to 127, the next free
  number in the breaker block. Keyed by *(number as written, name as written)*
  so the correction stops applying the moment the client fixes it upstream — a
  stale entry quietly rewriting a good row is the failure worth guarding
  against.
- **Reserved-placeholder consumption** — a named alarm landing on a reserved
  number takes over the blank placeholder there, rather than both surviving and
  re-raising the duplicate. This is why the breaker block is now `101–127`
  named and `128–135` reserved.

Raise these with the client so they can be fixed at source and the entries
retired.

```bash
# 1. Spreadsheet -> JSON. openpyxl required (pip install openpyxl). The source
#    .xlsx is kept outside the repo; point the generator at it. If a single
#    .xlsx sits in the project root (the dir containing neems-core) it is
#    picked up automatically.
ALARM_XLSX=/path/to/source.xlsx python3 neems-core/docs/alarms/build_alarm_spec.py

# 2. JSON -> Rust. Regenerates alarm_sld_meta.rs in place, already formatted to
#    the workspace rustfmt.toml (no `cargo fmt` step needed; same in, same out).
python3 neems-core/docs/alarms/build_alarm_meta_rs.py

# 3. JSON -> Rust. Regenerates analog_points.rs — the analog register map.
python3 neems-core/docs/alarms/build_analog_points_rs.py
```

The `test_every_definition_has_sld_meta` unit test fails if any
`ALARM_DEFINITIONS` entry loses its metadata after a regeneration, so CI catches
drift between the hand-curated Rust definitions and the spec. On the analog
side, `the_generated_registry_agrees_with_the_address_map` and
`the_named_offsets_point_at_the_measurements_they_claim` (in `protocol.rs`)
catch a regeneration that moves a point out from under the hand-written
`mp_analog_offset` constants.
