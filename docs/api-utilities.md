# Utility Endpoints

This document covers utility API endpoints in the neems-api system including health checks and location services.

See [api.md](api.md) for general information about the API including base URL, error handling, and authentication.

## Utility Endpoints

### Health Status

- **URL:** `/api/1/status`
- **Method:** `GET`
- **Purpose:** Returns the health status of the application
- **Authentication:** None required

#### Response

**Success (HTTP 200 OK):**
```json
{
  "status": "running"
}
```

#### Example

```js
const response = await fetch('/api/1/status');
const data = await response.json();
console.log(data.status); // "running"
```

### FixPhrase Encoding

- **URL:** `/api/1/fixphrase/encode/<lat>/<lon>`
- **Method:** `GET`
- **Purpose:** Encodes latitude/longitude coordinates into a FixPhrase string
- **Authentication:** None required
- **Feature Gate:** Only available with `fixphrase` feature

#### Parameters

- `lat` - Latitude coordinate (between -90 and 90)
- `lon` - Longitude coordinate (between -180 and 180)

#### Response

**Success (HTTP 200 OK):**
```json
{
  "phrase": "example.fixphrase.string",
  "latitude": 40.7128,
  "longitude": -74.0060,
  "accuracy": 10.0
}
```

**Failure (HTTP 400 Bad Request):**
```json
{
  "error": "Invalid coordinates"
}
```

#### Example

```js
const response = await fetch('/api/1/fixphrase/encode/40.7128/-74.0060');
const data = await response.json();
console.log(data.phrase); // "example.fixphrase.string"
```

## Utility System Overview

### Health Monitoring

The health status endpoint provides a simple way to monitor API availability:
- Returns 200 OK when the service is operational
- Can be used by load balancers and monitoring systems
- Minimal processing to ensure fast response

### FixPhrase Location Service

FixPhrase is a location encoding system that converts coordinates to readable phrases:
- Encodes latitude/longitude into human-readable strings
- Useful for location sharing and verification
- Returns both the encoded phrase and the decoded coordinates for verification
- Includes accuracy information

**Note:** The FixPhrase endpoint is only available when the `fixphrase` feature is enabled during compilation.

### Integration Notes

These utility endpoints are designed to be lightweight and have minimal dependencies:
- Health check requires no authentication or database access
- FixPhrase encoding is a pure computational function
- Both endpoints can be safely called frequently