# Data Access Endpoints

This document covers all data access API endpoints in the neems-api system for retrieving sensor readings and data source information.

See [api.md](api.md) for general information about the API including base URL, error handling, and authentication.

## Data Access

### OData Features

The DataSources endpoint supports OData v4 features:

- **Query Options**: Use `$select`, `$filter`, `$orderby`, `$top`, `$skip`, `$count`, and `$expand`
- **Collection Response Format**: Results are wrapped in OData envelope with `@odata.context`, `@odata.count`, and `value` properties

### List Data Sources

- **URL:** `/api/1/DataSources`
- **Method:** `GET`
- **Purpose:** Returns a list of all data sources in the database
- **Authentication:** Not required

#### Response

**Success (HTTP 200 OK):**
```json
{
  "@odata.context": "http://localhost/api/1/$metadata#DataSources", 
  "@odata.count": 1,
  "value": [
    {
      "id": 1,
      "name": "Temperature Sensor A",
      "description": "Main building temperature monitoring",
      "active": true,
      "interval_seconds": 300,
      "last_run": "2024-01-01T12:00:00",
      "created_at": "2024-01-01T00:00:00",
      "updated_at": "2024-01-01T00:00:00",
      "company_id": 1
    }
  ]
}
```

### Get Readings for Single Data Source

- **URL:** `/api/1/data/<source_id>`
- **Method:** `GET`
- **Purpose:** Returns readings for a specific data source with optional filtering
- **Authentication:** Required - users can only access readings from sources in their company

#### Parameters

- `source_id` - The ID of the source to retrieve readings from

#### Query Parameters

**Time Window (mutually exclusive with other options):**
- `since`: ISO 8601 timestamp (e.g., "2024-01-01T00:00:00Z") - start of time window
- `until`: ISO 8601 timestamp (e.g., "2024-01-02T00:00:00Z") - end of time window

**Count-based from timestamp (mutually exclusive):**
- `from_time`: ISO 8601 timestamp - start from this time
- `count`: Number of readings to return (1-10000)

**Count-based to timestamp (mutually exclusive):**
- `to_time`: ISO 8601 timestamp - end at this time  
- `count`: Number of readings to return (1-10000)

**Latest readings (mutually exclusive):**
- `latest`: Number of most recent readings (1-10000)

#### Authorization

- **Company Users**: Can only access readings from sources in their company
- **newtown-staff/newtown-admin**: Can access readings from any company

#### Response

**Success (HTTP 200 OK):**
```json
{
  "readings": [
    {
      "id": 1,
      "source_id": 123,
      "timestamp": "2024-01-01T12:00:00",
      "data": "{\"temperature\": 23.5}",
      "quality_flags": 0
    }
  ],
  "source_id": 123,
  "total_count": null
}
```

**Failure (HTTP 400 Bad Request):** Invalid query parameters

**Failure (HTTP 401 Unauthorized):** User not authenticated

**Failure (HTTP 403 Forbidden):** User lacks permission to access this source

**Failure (HTTP 404 Not Found):** Source ID does not exist

#### Examples

Get latest 100 readings:
```js
const response = await fetch('/api/1/data/123?latest=100', {
  method: 'GET',
  credentials: 'include'
});
```

Get readings in time window:
```js
const response = await fetch('/api/1/data/123?since=2024-01-01T00:00:00Z&until=2024-01-02T00:00:00Z', {
  method: 'GET',
  credentials: 'include'
});
```

Get 50 readings starting from a specific time:
```js
const response = await fetch('/api/1/data/123?from_time=2024-01-01T12:00:00Z&count=50', {
  method: 'GET',
  credentials: 'include'
});
```

### Get Readings for Multiple Data Sources

- **URL:** `/api/1/data/readings`
- **Method:** `GET`
- **Purpose:** Returns readings from multiple data sources with optional filtering
- **Authentication:** Required - users can only access readings from sources in their company

#### Query Parameters

**Required:**
- `source_ids`: Comma-separated list of source IDs (e.g., "1,2,3")

**Time filtering (same as single source endpoint):**
- `since`/`until`: Time window
- `from_time`/`count`: Count-based from timestamp  
- `to_time`/`count`: Count-based to timestamp
- `latest`: Number of most recent readings per source

#### Authorization

- **Company Users**: Can only access readings from sources in their company
- **newtown-staff/newtown-admin**: Can access readings from any company
- All requested source IDs must be accessible to the user or the request fails

#### Response

**Success (HTTP 200 OK):**
```json
{
  "readings": [
    {
      "id": 1,
      "source_id": 1,
      "timestamp": "2024-01-01T12:00:00", 
      "data": "{\"temperature\": 23.5}",
      "quality_flags": 0
    },
    {
      "id": 2,
      "source_id": 2,
      "timestamp": "2024-01-01T12:00:00",
      "data": "{\"humidity\": 45.2}",
      "quality_flags": 0
    }
  ],
  "source_id": null,
  "total_count": null
}
```

**Failure (HTTP 400 Bad Request):** Invalid query parameters or missing source_ids

**Failure (HTTP 401 Unauthorized):** User not authenticated

**Failure (HTTP 403 Forbidden):** User lacks permission to access one or more sources

**Failure (HTTP 404 Not Found):** One or more source IDs do not exist

#### Example

Get latest readings from multiple sources:
```js
const response = await fetch('/api/1/data/readings?source_ids=1,2,3&latest=10', {
  method: 'GET',
  credentials: 'include'
});
```

### Get Site Database Schema (Test/Staging Only)

- **URL:** `/api/1/data/schema`
- **Method:** `GET`
- **Purpose:** Returns the SQLite database schema as JSON
- **Authentication:** Not required
- **Feature Gate:** Only available with `test-staging` feature

**Note:** This endpoint is only available when the `test-staging` feature is enabled during compilation. See [api-testing.md](api-testing.md) for more test-staging endpoints.

#### Response

**Success (HTTP 200 OK):**
```json
{
  "schema": "CREATE TABLE sources (...); CREATE TABLE readings (...);"
}
```

## Data System Overview

### Data Sources

Data sources represent sensors or other data collection points:
- Each source has a unique ID
- Sources can be associated with companies for access control
- Sources have metadata like name, description, and collection interval

### Readings

Readings are time-series data points from sources:
- Each reading has a timestamp
- Data is stored as JSON in the `data` field
- Quality flags indicate data reliability
- Readings are linked to their source

### Query Optimization

The API provides several query patterns optimized for different use cases:
- **Latest**: Get the most recent N readings
- **Time window**: Get all readings between two timestamps
- **Count from/to**: Get a specific number of readings from/to a point in time

### Access Control

Data access is controlled at the source level:
- Users can only access readings from sources in their company
- Newtown staff/admin can access all sources
- Authorization is checked before any data is returned