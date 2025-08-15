# Device Management API

This document describes the Device Management endpoints for managing physical devices within sites. Devices represent physical hardware like sensors, meters, inverters, and other equipment that are monitored or controlled by the NEEMS system.

## Base URL

All endpoints are prefixed with `/api/1/`

## Authentication

All device endpoints require authentication via session cookies. See [Authentication](api-auth.md) for details.

## Device Model

### Device Entity

```typescript
interface Device {
  id: number;                           // Unique device identifier
  name: string;                         // Device name
  description?: string;                 // Optional device description
  type_: string;                        // Device type (e.g., "Inverter", "Meter", "Battery")
  model: string;                        // Device model name
  serial?: string;                      // Optional serial number
  ip_address?: string;                  // Optional IP address for networked devices
  install_date?: string | null;         // Optional installation date (ISO 8601)
  company_id: number;                   // ID of owning company
  site_id: number;                      // ID of site where device is located
}
```

### Device Input

```typescript
interface DeviceInput {
  name?: string;                        // Optional name (defaults to type if not provided)
  description?: string;                 // Optional description
  type_: string;                        // Required device type
  model: string;                        // Required model name
  serial?: string;                      // Optional serial number
  ip_address?: string;                  // Optional IP address
  install_date?: string | null;         // Optional installation date
  company_id: number;                   // Required company ID
  site_id: number;                      // Required site ID
}
```

### Device with Timestamps

```typescript
interface DeviceWithTimestamps {
  id: number;
  name: string;
  description?: string;
  type_: string;
  model: string;
  serial?: string;
  ip_address?: string;
  install_date?: string | null;
  company_id: number;
  site_id: number;
  created_at: string;                   // Creation timestamp (ISO 8601)
  updated_at: string;                   // Last update timestamp (ISO 8601)
}
```

## Common Device Types

The system supports various device types commonly found in energy monitoring systems:

- **Inverter** - Solar/PV inverters
- **Meter** - Power/energy meters  
- **Battery** - Energy storage systems
- **Protection** - Protection relays and devices
- **UPS** - Uninterruptible power supplies
- **Sensor** - Environmental or electrical sensors
- **Gateway** - Communication gateways
- **Controller** - System controllers

## Device Hierarchy

Devices exist within the following hierarchy:

```
Company
  └── Site
      └── Device
```

- Each device belongs to exactly one **Site**
- Each device belongs to exactly one **Company** (inherited from site)
- A site can contain multiple devices
- Device names must be unique within their site

## Field Descriptions

### Required Fields

- **`type_`**: The category of device (e.g., "Inverter", "Meter", "Battery")
- **`model`**: The specific model name or number
- **`company_id`**: Must reference an existing company
- **`site_id`**: Must reference an existing site belonging to the specified company

### Optional Fields

- **`name`**: Human-readable device name. If not provided, defaults to the device type
- **`description`**: Additional details about the device
- **`serial`**: Manufacturer serial number for tracking and support
- **`ip_address`**: Network address for devices that communicate over IP
- **`install_date`**: When the device was physically installed

## Business Rules

### Name Generation
- If no name is provided, the device name defaults to its type
- Device names must be unique within their site
- Names are case-insensitive for uniqueness checking but preserve original case

### Site and Company Validation
- The specified site must exist and belong to the specified company
- Users can only create devices for companies they have access to
- Device location is tied to its site's physical location

### Data Relationships
- Devices may have associated sensor readings and data sources
- Devices are automatically removed when their parent site is deleted
- Device deletion may affect associated monitoring data

## Activity Tracking

The system tracks device lifecycle events:

- **Creation**: When a device is first registered
- **Updates**: When device properties are modified
- **Installation**: Based on the install_date field
- **Data Collection**: Associated with readings and monitoring

Activity timestamps can be retrieved using OData `$select` queries:

```bash
GET /api/1/Devices?$select=id,name,type_,activity_created_at,activity_updated_at
```

## Error Handling

### Common Error Responses

**Validation Errors:**
```json
{
  "error": "Device name already exists in this site"
}
```

**Not Found:**
```json
{
  "error": "Site not found or access denied",
  "status": 404,
  "path": "/api/1/Devices"
}
```

**Permission Denied:**
```json
{
  "error": "Insufficient permissions to access this company's devices",
  "status": 403,
  "path": "/api/1/Devices"
}
```

## OData Query Support

Device endpoints support standard OData query options:

### Filtering Examples

```bash
# Get devices by type
GET /api/1/Devices?$filter=type_ eq 'Inverter'

# Get devices at a specific site
GET /api/1/Devices?$filter=site_id eq 123

# Get devices by company
GET /api/1/Devices?$filter=company_id eq 1

# Get devices installed after a date
GET /api/1/Devices?$filter=install_date ge 2024-01-01T00:00:00Z

# Get devices with IP addresses
GET /api/1/Devices?$filter=ip_address ne null

# Search by model or serial
GET /api/1/Devices?$filter=contains(model,'SUN2000') or contains(serial,'INV001')
```

### Sorting Examples

```bash
# Sort by installation date
GET /api/1/Devices?$orderby=install_date desc

# Sort by site, then by name
GET /api/1/Devices?$orderby=site_id,name

# Sort by device type and model
GET /api/1/Devices?$orderby=type_,model
```

### Field Selection

```bash
# Get basic device info
GET /api/1/Devices?$select=id,name,type_,model

# Include location info
GET /api/1/Devices?$select=id,name,site_id,company_id

# Include timestamps
GET /api/1/Devices?$select=id,name,activity_created_at,activity_updated_at
```

### Related Data

```bash
# Include site information
GET /api/1/Devices?$expand=Site

# Include company information via site
GET /api/1/Devices?$expand=Site($expand=Company)
```

## Future API Endpoints

*Note: Device API endpoints are planned but not yet implemented. The following endpoints will be available in future releases:*

### Collection Operations
- `GET /api/1/Devices` - List all devices with filtering and pagination
- `POST /api/1/Devices` - Create a new device

### Individual Device Operations  
- `GET /api/1/Devices/{id}` - Get device by ID
- `PUT /api/1/Devices/{id}` - Update device
- `DELETE /api/1/Devices/{id}` - Delete device

### Navigation Properties
- `GET /api/1/Devices/{id}/Site` - Get device's site
- `GET /api/1/Sites/{id}/Devices` - Get all devices at a site
- `GET /api/1/Companies/{id}/Devices` - Get all devices for a company

## Usage Examples

### Creating a Solar Inverter

```typescript
const deviceData: DeviceInput = {
  name: "Main Inverter",
  description: "Primary solar inverter for building roof array",
  type_: "Inverter", 
  model: "SUN2000-100KTL",
  serial: "INV20240001",
  ip_address: "192.168.1.100",
  install_date: "2024-03-15T10:00:00Z",
  company_id: 1,
  site_id: 5
};

const response = await fetch('/api/1/Devices', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify(deviceData),
  credentials: 'include'
});
```

### Querying Site Devices

```typescript
// Get all inverters at a specific site
const response = await fetch(
  '/api/1/Devices?$filter=site_id eq 5 and type_ eq \'Inverter\'&$orderby=name',
  { credentials: 'include' }
);

const data = await response.json();
const inverters = data.value; // Array of Device objects
```

## Related Documentation

- [Site Management](api-sites.md) - Managing physical locations where devices are installed
- [Company Management](api-companies.md) - Managing organizations that own devices  
- [Data Access](api-data.md) - Accessing readings and data from devices
- [Authentication](api-auth.md) - Required for all device operations