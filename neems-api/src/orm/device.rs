use diesel::prelude::*;

use crate::models::{Device, DeviceInput, DeviceWithTimestamps, NewDevice};

/// Gets all devices for a specific site.
pub fn get_devices_by_site(
    conn: &mut SqliteConnection,
    device_site_id: i32,
) -> Result<Vec<Device>, diesel::result::Error> {
    use crate::schema::devices::dsl::*;
    devices
        .filter(crate::schema::devices::site_id.eq(device_site_id))
        .order(id.asc())
        .select(Device::as_select())
        .load(conn)
}

/// Gets all devices for a specific company.
pub fn get_devices_by_company(
    conn: &mut SqliteConnection,
    comp_id: i32,
) -> Result<Vec<Device>, diesel::result::Error> {
    use crate::schema::devices::dsl::*;
    devices
        .filter(company_id.eq(comp_id))
        .order(id.asc())
        .select(Device::as_select())
        .load(conn)
}

/// Creates a new device in the database (timestamps handled automatically by
/// database triggers) If name is not provided, it defaults to the device type
pub fn insert_device(
    conn: &mut SqliteConnection,
    device_input: DeviceInput,
    acting_user_id: Option<i32>,
) -> Result<Device, diesel::result::Error> {
    use crate::schema::devices::dsl::*;

    // Use provided name or default to type
    let device_name = device_input.name.unwrap_or_else(|| device_input.type_.clone());

    let new_device = NewDevice {
        name: device_name,
        description: device_input.description,
        type_: device_input.type_,
        model: device_input.model,
        serial: device_input.serial,
        ip_address: device_input.ip_address,
        install_date: device_input.install_date,
        company_id: device_input.company_id,
        site_id: device_input.site_id,
    };

    diesel::insert_into(devices).values(&new_device).execute(conn)?;

    // Return the inserted device
    let device = devices.order(id.desc()).select(Device::as_select()).first(conn)?;

    // Update the trigger-created activity entry with user information
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(conn, "devices", device.id, "create", user_id);
    }

    Ok(device)
}

/// Gets a device by its ID.
pub fn get_device_by_id(
    conn: &mut SqliteConnection,
    device_id: i32,
) -> Result<Option<Device>, diesel::result::Error> {
    use crate::schema::devices::dsl::*;
    devices
        .filter(id.eq(device_id))
        .select(Device::as_select())
        .first(conn)
        .optional()
}

/// Gets a device by site ID and name (case-insensitive).
pub fn get_device_by_site_and_name(
    conn: &mut SqliteConnection,
    device_site_id: i32,
    device_name: &str,
) -> Result<Option<Device>, diesel::result::Error> {
    // Use raw SQL for case-insensitive comparison
    diesel::sql_query("SELECT id, name, description, type as type_, model, serial, ip_address, install_date, company_id, site_id FROM devices WHERE site_id = ? AND LOWER(name) = LOWER(?)")
        .bind::<diesel::sql_types::Integer, _>(device_site_id)
        .bind::<diesel::sql_types::Text, _>(device_name)
        .get_result::<Device>(conn)
        .optional()
}

/// Gets all devices in the system.
pub fn get_all_devices(conn: &mut SqliteConnection) -> Result<Vec<Device>, diesel::result::Error> {
    use crate::schema::devices::dsl::*;
    devices.order(id.asc()).select(Device::as_select()).load(conn)
}

/// Updates a device in the database (timestamps handled automatically by
/// database triggers)
pub fn update_device(
    conn: &mut SqliteConnection,
    device_id: i32,
    new_name: Option<String>,
    new_description: Option<Option<String>>, /* Double Option to distinguish between "don't
                                              * change" and "set to null" */
    new_type: Option<String>,
    new_model: Option<String>,
    new_serial: Option<Option<String>>,
    new_ip_address: Option<Option<String>>,
    new_install_date: Option<Option<chrono::NaiveDateTime>>,
    new_company_id: Option<i32>,
    new_site_id: Option<i32>,
    acting_user_id: Option<i32>,
) -> Result<Device, diesel::result::Error> {
    use crate::schema::devices::dsl::*;

    // First, get the current device to preserve existing values
    let current_device =
        devices.filter(id.eq(device_id)).select(Device::as_select()).first(conn)?;

    // Update with new values or keep existing ones
    diesel::update(devices.filter(id.eq(device_id)))
        .set((
            name.eq(new_name.unwrap_or(current_device.name)),
            description.eq(new_description.unwrap_or(current_device.description)),
            type_.eq(new_type.unwrap_or(current_device.type_)),
            model.eq(new_model.unwrap_or(current_device.model)),
            serial.eq(new_serial.unwrap_or(current_device.serial)),
            ip_address.eq(new_ip_address.unwrap_or(current_device.ip_address)),
            install_date.eq(new_install_date.unwrap_or(current_device.install_date)),
            company_id.eq(new_company_id.unwrap_or(current_device.company_id)),
            site_id.eq(new_site_id.unwrap_or(current_device.site_id)),
        ))
        .execute(conn)?;

    // Return the updated device
    let device = devices.filter(id.eq(device_id)).select(Device::as_select()).first(conn)?;

    // Update the trigger-created activity entry with user information
    if let Some(user_id) = acting_user_id {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(conn, "devices", device_id, "update", user_id);
    }

    Ok(device)
}

/// Deletes a device from the database.
pub fn delete_device(
    conn: &mut SqliteConnection,
    device_id: i32,
    acting_user_id: Option<i32>,
) -> Result<usize, diesel::result::Error> {
    use crate::schema::devices::dsl::*;
    let result = diesel::delete(devices.filter(id.eq(device_id))).execute(conn)?;

    // Update the trigger-created activity entry with user information
    if result > 0
        && let Some(user_id) = acting_user_id
    {
        use crate::orm::entity_activity::update_latest_activity_user;
        let _ = update_latest_activity_user(conn, "devices", device_id, "delete", user_id);
    }

    Ok(result)
}

/// Get a device with computed timestamps from activity log
pub fn get_device_with_timestamps(
    conn: &mut SqliteConnection,
    device_id: i32,
) -> Result<Option<DeviceWithTimestamps>, diesel::result::Error> {
    use crate::orm::entity_activity;

    // First get the device
    let device = match get_device_by_id(conn, device_id)? {
        Some(d) => d,
        None => return Ok(None),
    };

    // Get timestamps from activity log
    let created_at = entity_activity::get_created_at(conn, "devices", device_id)?;
    let updated_at = entity_activity::get_updated_at(conn, "devices", device_id)?;

    Ok(Some(DeviceWithTimestamps {
        id: device.id,
        name: device.name,
        description: device.description,
        type_: device.type_,
        model: device.model,
        serial: device.serial,
        ip_address: device.ip_address,
        install_date: device.install_date,
        company_id: device.company_id,
        site_id: device.site_id,
        created_at,
        updated_at,
    }))
}

#[cfg(test)]
mod tests;
