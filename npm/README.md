# @newtown-energy/types

TypeScript type definitions for the Newtown Energy Management System (NEEMS) API.

These types are **auto-generated** from the Rust backend using [ts-rs](https://github.com/Aleph-Alpha/ts-rs) and published automatically when the API types change.

## Installation

```bash
npm install @newtown-energy/types
```

## Usage

```typescript
import type { Battery, Schedule, BatteryStatus } from "@newtown-energy/types";
```

## Versioning

The package version mirrors the `neems-api` crate version in the backend repository. Types follow semver:

- **Patch**: compatible additions (new optional fields, new types)
- **Minor**: new types or endpoints that don't break existing consumers
- **Major**: breaking changes (renamed/removed fields, changed type shapes)

## Source

These types are generated from the [neems-core](https://github.com/Newtown-Energy/neems-core) repository. Do not edit them manually.
