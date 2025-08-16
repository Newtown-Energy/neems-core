# Scheduler API Documentation

The Scheduler API provides comprehensive automation capabilities for site state management through user-defined Lua scripts and manual overrides.

## Overview

The scheduler system allows users to:
- **Create Lua scripts** that dynamically determine site states (charge, discharge, idle) based on time, weather, or other conditions
- **Set manual overrides** for maintenance or emergency situations that temporarily supersede script logic
- **Execute and validate** scripts with built-in sandboxing and security measures
- **Monitor execution history** and track state changes over time

### State Resolution Priority
1. **Active Overrides** - Manual overrides within their time window (highest priority)
2. **Active Scripts** - Latest active script for the site
3. **Default State** - Idle state when no scripts or overrides apply

## Entity Sets

### SchedulerScripts
User-defined Lua scripts that determine site states based on datetime and site data.

### SchedulerOverrides  
Manual state overrides with time-based scheduling for maintenance or emergency situations.

### SchedulerExecutions
Execution history and logging for scheduler operations (read-only).

## SchedulerScripts Endpoints

### Create Scheduler Script
Create a new Lua script for automated site state management.

**Endpoint:** `POST /api/1/SchedulerScripts`

**Authorization:** Company admin or newtown role required

**Request Body:**
```json
{
  "site_id": 1,
  "name": "Solar Charging Schedule",
  "script_content": "if datetime.hour >= 9 and datetime.hour < 17 then return 'charge' else return 'idle' end",
  "language": "lua",
  "is_active": true,
  "version": 1
}
```

**Response:** `201 Created`
```json
{
  "id": 123,
  "site_id": 1,
  "name": "Solar Charging Schedule",
  "script_content": "if datetime.hour >= 9 and datetime.hour < 17 then return 'charge' else return 'idle' end",
  "language": "lua",
  "is_active": true,
  "version": 1
}
```

**Validation:**
- Script size limited to 10KB
- Script name must be unique per site
- Only "lua" language supported
- Content validates for basic Lua syntax

### List Scheduler Scripts
Get all scheduler scripts with OData query support.

**Endpoint:** `GET /api/1/SchedulerScripts`

**Query Parameters:**
- Standard OData options: `$select`, `$filter`, `$orderby`, `$top`, `$skip`, `$count`, `$expand`
- Filter by site: `$filter=site_id eq 1`
- Active scripts only: `$filter=is_active eq true`

**Response:** `200 OK`
```json
{
  "@odata.context": "http://localhost/api/1/$metadata#SchedulerScripts",
  "@odata.count": 5,
  "value": [
    {
      "id": 123,
      "site_id": 1,
      "name": "Solar Charging Schedule",
      "script_content": "if datetime.hour >= 9 and datetime.hour < 17 then return 'charge' else return 'idle' end",
      "language": "lua",
      "is_active": true,
      "version": 1
    }
  ]
}
```

### Get Scheduler Script
Retrieve a specific scheduler script by ID.

**Endpoint:** `GET /api/1/SchedulerScripts/{script_id}`

**Response:** `200 OK`
```json
{
  "id": 123,
  "site_id": 1,
  "name": "Solar Charging Schedule",
  "script_content": "if datetime.hour >= 9 and datetime.hour < 17 then return 'charge' else return 'idle' end",
  "language": "lua",
  "is_active": true,
  "version": 1
}
```

### Update Scheduler Script
Update an existing scheduler script.

**Endpoint:** `PUT /api/1/SchedulerScripts/{script_id}`

**Authorization:** Company admin or newtown role required

**Request Body:**
```json
{
  "name": "Updated Solar Schedule",
  "script_content": "-- Updated logic\nif datetime.hour >= 8 and datetime.hour < 18 then return 'charge' else return 'idle' end",
  "is_active": false
}
```

**Response:** `200 OK` - Updated script object

### Delete Scheduler Script
Remove a scheduler script.

**Endpoint:** `DELETE /api/1/SchedulerScripts/{script_id}`

**Authorization:** Company admin or newtown role required

**Response:** `204 No Content`

### Validate Scheduler Script
Validate a script for syntax errors without executing it.

**Endpoint:** `POST /api/1/SchedulerScripts/{script_id}/validate`

**Authorization:** Company admin or newtown role required

**Response:** `200 OK`
```json
{
  "is_valid": true,
  "error": null,
  "test_state": "charge",
  "execution_time_ms": 2
}
```

**Error Response:**
```json
{
  "is_valid": false,
  "error": "Script compilation failed: syntax error near 'end'",
  "test_state": null,
  "execution_time_ms": null
}
```

## SchedulerOverrides Endpoints

### Create Scheduler Override
Create a manual override for a specific time period.

**Endpoint:** `POST /api/1/SchedulerOverrides`

**Authorization:** Company admin or newtown role required

**Request Body:**
```json
{
  "site_id": 1,
  "state": "discharge",
  "start_time": "2024-08-16T10:00:00",
  "end_time": "2024-08-16T14:00:00",
  "reason": "Emergency load shedding",
  "is_active": true
}
```

**Response:** `201 Created`
```json
{
  "id": 456,
  "site_id": 1,
  "state": "discharge",
  "start_time": "2024-08-16T10:00:00",
  "end_time": "2024-08-16T14:00:00",
  "reason": "Emergency load shedding",
  "is_active": true
}
```

**Validation:**
- `state` must be one of: "charge", "discharge", "idle"
- `end_time` must be after `start_time`
- No overlapping active overrides for the same site
- Time format: ISO 8601 (`YYYY-MM-DDTHH:MM:SS`)

### List Scheduler Overrides
Get all scheduler overrides with OData query support.

**Endpoint:** `GET /api/1/SchedulerOverrides`

**Query Parameters:**
- Standard OData options supported
- Filter by site: `$filter=site_id eq 1`
- Active overrides: `$filter=is_active eq true`
- Current overrides: `$filter=start_time le now() and end_time ge now()`

**Response:** `200 OK` - OData collection of overrides

## Site Navigation Endpoints

### Get Site Scheduler Scripts
Get all scheduler scripts for a specific site.

**Endpoint:** `GET /api/1/Sites/{site_id}/SchedulerScripts`

**Response:** `200 OK` - Array of scripts for the site

### Get Site Scheduler Overrides
Get all scheduler overrides for a specific site.

**Endpoint:** `GET /api/1/Sites/{site_id}/SchedulerOverrides`

**Response:** `200 OK` - Array of overrides for the site

## Scheduler Execution Endpoints

### Get Site State
Get the current resolved state for a site.

**Endpoint:** `GET /api/1/Sites/{site_id}/scheduler/state`

**Query Parameters:**
- `datetime` (optional): ISO 8601 timestamp for historical state lookup
- If not provided, uses current time

**Response:** `200 OK`
```json
{
  "site_id": 1,
  "state": "charge",
  "source": "script:123",
  "execution_time_ms": 5,
  "error": null,
  "resolved_at": "2024-08-16T12:30:00Z"
}
```

**Source Values:**
- `"default"` - No active scripts or overrides
- `"script:{id}"` - State determined by script with given ID
- `"override:{id}"` - State determined by override with given ID

### Execute Site Scheduler
Manually trigger scheduler execution for a site and log the result.

**Endpoint:** `POST /api/1/Sites/{site_id}/scheduler/execute`

**Authorization:** Company admin or newtown role required

**Request Body:**
```json
{
  "datetime": "2024-08-16T12:30:00Z"
}
```

**Response:** `200 OK`
```json
{
  "site_id": 1,
  "state": "charge",
  "source": "script:123",
  "execution_time_ms": 5,
  "error": null,
  "executed_at": "2024-08-16T12:30:00Z",
  "execution_id": 789
}
```

## Lua Script Environment

### Available Globals

Scheduler scripts execute in a sandboxed Lua environment with the following global variables:

#### `datetime` Table
Current execution time with the following properties:
```lua
{
  year = 2024,        -- Full year
  month = 8,          -- Month (1-12)
  day = 16,           -- Day of month (1-31)
  hour = 14,          -- Hour (0-23)
  minute = 30,        -- Minute (0-59)
  second = 0,         -- Second (0-59)
  weekday = 5,        -- Day of week (1=Monday, 7=Sunday)
  timestamp = 1692196200  -- Unix timestamp
}
```

#### `site_data` Table
Information about the site:
```lua
{
  id = 1,
  name = "Main Solar Farm",
  company_id = 2,
  latitude = 40.7128,   -- Optional
  longitude = -74.0060  -- Optional
}
```

### Return Values
Scripts must return one of the following string values:
- `"charge"` - Site should be in charging state
- `"discharge"` - Site should be in discharging state  
- `"idle"` - Site should be in idle state

### Example Scripts

**Basic Time-Based Charging:**
```lua
-- Charge during daylight hours (9 AM to 5 PM)
if datetime.hour >= 9 and datetime.hour < 17 then
  return 'charge'
else
  return 'idle'
end
```

**Weekend vs Weekday Logic:**
```lua
-- Different behavior on weekends
if datetime.weekday >= 6 then  -- Saturday or Sunday
  return 'idle'
elseif datetime.hour >= 8 and datetime.hour < 20 then
  return 'charge'
else
  return 'discharge'
end
```

**Complex Conditional Logic:**
```lua
-- Peak shaving during high-cost hours
local is_peak_hours = (datetime.hour >= 16 and datetime.hour < 20)
local is_weekend = datetime.weekday >= 6
local is_summer = datetime.month >= 6 and datetime.month <= 8

if is_weekend then
  return 'idle'
elseif is_peak_hours and is_summer then
  return 'discharge'  -- Avoid peak rates
elseif datetime.hour >= 10 and datetime.hour < 16 then
  return 'charge'     -- Charge during solar hours
else
  return 'idle'
end
```

### Security & Limitations

**Sandboxing:**
- Dangerous modules removed: `io`, `os`, `package`, `debug`, `require`
- No file system or network access
- No ability to load external code

**Execution Limits:**
- **Timeout:** 100ms maximum execution time
- **Size:** 10KB maximum script size
- **Memory:** Limited by Lua sandbox

**Allowed Operations:**
- Basic arithmetic and logic
- String manipulation
- Table operations
- Control flow (if/then/else, loops)
- Mathematical functions

## Error Handling

### Common Error Responses

**Script Validation Errors:**
```json
{
  "error": "Script compilation failed: syntax error near 'end'",
  "status": 400
}
```

**Authorization Errors:**
```json
{
  "error": "Forbidden: Company admin or newtown role required",
  "status": 403
}
```

**Conflict Errors:**
```json
{
  "error": "Overlapping override exists for this time period",
  "status": 409
}
```

**Execution Timeout:**
```json
{
  "error": "Script execution timed out after 100ms",
  "status": 500
}
```

### Script Runtime Errors
Runtime errors in Lua scripts are captured and returned in execution responses:

```json
{
  "site_id": 1,
  "state": "idle",
  "source": "default",
  "execution_time_ms": 50,
  "error": "Script execution error: attempt to index a nil value",
  "executed_at": "2024-08-16T12:30:00Z"
}
```

## Best Practices

### Script Development
1. **Test thoroughly** with the validation endpoint before deploying
2. **Keep scripts simple** to avoid timeout issues
3. **Handle edge cases** gracefully (missing data, unexpected times)
4. **Use meaningful variable names** for maintainability
5. **Add comments** to explain complex logic

### Override Management
1. **Use specific time windows** to avoid conflicts
2. **Provide clear reasons** for audit trails
3. **Clean up expired overrides** periodically
4. **Coordinate with team** to avoid overlapping overrides

### Security Considerations
1. **Validate all inputs** before script creation
2. **Monitor execution logs** for suspicious activity
3. **Use least privilege** - only grant necessary permissions
4. **Regular audits** of active scripts and overrides

### Performance Tips
1. **Cache site state** when possible instead of frequent execution calls
2. **Use appropriate polling intervals** for state checks
3. **Monitor execution times** and optimize slow scripts
4. **Batch operations** when updating multiple sites

## TypeScript Types

Generated TypeScript types are available for all scheduler-related data structures:

```typescript
import { 
  SchedulerScript,
  SchedulerScriptInput,
  UpdateSchedulerScriptRequest,
  SchedulerOverride,
  SchedulerOverrideInput,
  SiteState,
  ValidateScriptResponse,
  ExecuteSchedulerResponse,
  SiteStateResponse
} from './generated-types';

// Type-safe script creation
const newScript: SchedulerScriptInput = {
  site_id: 1,
  name: "Solar Schedule",
  script_content: "return 'charge'",
  language: "lua",
  is_active: true,
  version: 1
};

// Type-safe state handling
const currentState: SiteStateResponse = await fetch('/api/1/Sites/1/scheduler/state')
  .then(res => res.json());

if (currentState.state === 'charge') {
  // Handle charging state
}
```

For complete type definitions, see the generated TypeScript files in your project's types directory.