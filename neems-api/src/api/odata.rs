//! OData-specific endpoints and functionality.
//!
//! This module provides OData standard endpoints including metadata service
//! and service document, as well as support for OData query options.

use rocket::Route;
use rocket::response::content::RawXml;
use rocket::serde::json::Json;
use serde::Serialize;
use ts_rs::TS;

/// Service document listing available entity sets
#[derive(Serialize, TS)]
#[ts(export)]
pub struct ServiceDocument {
    #[serde(rename = "@odata.context")]
    pub context: String,
    pub value: Vec<EntitySet>,
}

/// Entity set information
#[derive(Serialize, TS)]
#[ts(export)]
pub struct EntitySet {
    pub name: String,
    pub kind: String,
    pub url: String,
}

/// OData Service Document endpoint.
///
/// - **URL:** `/api/1/`
/// - **Method:** `GET`
/// - **Purpose:** Returns the service document listing all available entity sets
/// - **Authentication:** None required
///
/// This endpoint provides the entry point for OData clients to discover
/// available entity sets and their URLs.
#[get("/1")]
pub fn service_document() -> Json<ServiceDocument> {
    Json(ServiceDocument {
        context: "http://localhost/api/1/$metadata".to_string(),
        value: vec![
            EntitySet {
                name: "Users".to_string(),
                kind: "EntitySet".to_string(),
                url: "Users".to_string(),
            },
            EntitySet {
                name: "Companies".to_string(),
                kind: "EntitySet".to_string(),
                url: "Companies".to_string(),
            },
            EntitySet {
                name: "Sites".to_string(),
                kind: "EntitySet".to_string(),
                url: "Sites".to_string(),
            },
            EntitySet {
                name: "Devices".to_string(),
                kind: "EntitySet".to_string(),
                url: "Devices".to_string(),
            },
            EntitySet {
                name: "Roles".to_string(),
                kind: "EntitySet".to_string(),
                url: "Roles".to_string(),
            },
            EntitySet {
                name: "DataSources".to_string(),
                kind: "EntitySet".to_string(),
                url: "DataSources".to_string(),
            },
            EntitySet {
                name: "Readings".to_string(),
                kind: "EntitySet".to_string(),
                url: "Readings".to_string(),
            },
            EntitySet {
                name: "SchedulerScripts".to_string(),
                kind: "EntitySet".to_string(),
                url: "SchedulerScripts".to_string(),
            },
            EntitySet {
                name: "SchedulerOverrides".to_string(),
                kind: "EntitySet".to_string(),
                url: "SchedulerOverrides".to_string(),
            },
        ],
    })
}

/// OData Metadata Document endpoint.
///
/// - **URL:** `/api/1/$metadata`
/// - **Method:** `GET`
/// - **Purpose:** Returns the Entity Data Model (EDM) describing the service
/// - **Authentication:** None required
///
/// This endpoint provides machine-readable metadata about the data model
/// including entity types, relationships, and operations.
#[get("/1/$metadata")]
pub fn metadata_document() -> RawXml<String> {
    let metadata = r#"<?xml version="1.0" encoding="utf-8"?>
<edmx:Edmx Version="4.0" xmlns:edmx="http://docs.oasis-open.org/odata/ns/edmx">
  <edmx:DataServices>
    <Schema Namespace="NeemsAPI" xmlns="http://docs.oasis-open.org/odata/ns/edm">
      
      <!-- User Entity Type -->
      <EntityType Name="User">
        <Key>
          <PropertyRef Name="id"/>
        </Key>
        <Property Name="id" Type="Edm.Int32" Nullable="false"/>
        <Property Name="email" Type="Edm.String" Nullable="false"/>
        <Property Name="password_hash" Type="Edm.String" Nullable="false"/>
        <Property Name="company_id" Type="Edm.Int32" Nullable="false"/>
        <Property Name="totp_secret" Type="Edm.String" Nullable="true"/>
        <Property Name="created_at" Type="Edm.DateTimeOffset" Nullable="false"/>
        <Property Name="updated_at" Type="Edm.DateTimeOffset" Nullable="false"/>
        <Property Name="activity_created_at" Type="Edm.DateTimeOffset" Nullable="true"/>
        <Property Name="activity_updated_at" Type="Edm.DateTimeOffset" Nullable="true"/>
        <NavigationProperty Name="Company" Type="NeemsAPI.Company" Nullable="false">
          <ReferentialConstraint Property="company_id" ReferencedProperty="id"/>
        </NavigationProperty>
        <NavigationProperty Name="Roles" Type="Collection(NeemsAPI.Role)"/>
      </EntityType>

      <!-- Company Entity Type -->
      <EntityType Name="Company">
        <Key>
          <PropertyRef Name="id"/>
        </Key>
        <Property Name="id" Type="Edm.Int32" Nullable="false"/>
        <Property Name="name" Type="Edm.String" Nullable="false"/>
        <Property Name="created_at" Type="Edm.DateTimeOffset" Nullable="false"/>
        <Property Name="updated_at" Type="Edm.DateTimeOffset" Nullable="false"/>
        <NavigationProperty Name="Users" Type="Collection(NeemsAPI.User)"/>
        <NavigationProperty Name="Sites" Type="Collection(NeemsAPI.Site)"/>
      </EntityType>

      <!-- Site Entity Type -->
      <EntityType Name="Site">
        <Key>
          <PropertyRef Name="id"/>
        </Key>
        <Property Name="id" Type="Edm.Int32" Nullable="false"/>
        <Property Name="name" Type="Edm.String" Nullable="false"/>
        <Property Name="address" Type="Edm.String" Nullable="true"/>
        <Property Name="latitude" Type="Edm.Double" Nullable="true"/>
        <Property Name="longitude" Type="Edm.Double" Nullable="true"/>
        <Property Name="company_id" Type="Edm.Int32" Nullable="false"/>
        <Property Name="created_at" Type="Edm.DateTimeOffset" Nullable="false"/>
        <Property Name="updated_at" Type="Edm.DateTimeOffset" Nullable="false"/>
        <NavigationProperty Name="Company" Type="NeemsAPI.Company" Nullable="false">
          <ReferentialConstraint Property="company_id" ReferencedProperty="id"/>
        </NavigationProperty>
        <NavigationProperty Name="SchedulerScripts" Type="Collection(NeemsAPI.SchedulerScript)"/>
        <NavigationProperty Name="SchedulerOverrides" Type="Collection(NeemsAPI.SchedulerOverride)"/>
      </EntityType>

      <!-- Role Entity Type -->
      <EntityType Name="Role">
        <Key>
          <PropertyRef Name="id"/>
        </Key>
        <Property Name="id" Type="Edm.Int32" Nullable="false"/>
        <Property Name="name" Type="Edm.String" Nullable="false"/>
        <Property Name="description" Type="Edm.String" Nullable="true"/>
        <Property Name="created_at" Type="Edm.DateTimeOffset" Nullable="false"/>
        <Property Name="updated_at" Type="Edm.DateTimeOffset" Nullable="false"/>
        <NavigationProperty Name="Users" Type="Collection(NeemsAPI.User)"/>
      </EntityType>

      <!-- DataSource Entity Type -->
      <EntityType Name="DataSource">
        <Key>
          <PropertyRef Name="id"/>
        </Key>
        <Property Name="id" Type="Edm.Int32" Nullable="false"/>
        <Property Name="name" Type="Edm.String" Nullable="false"/>
        <Property Name="description" Type="Edm.String" Nullable="true"/>
        <Property Name="active" Type="Edm.Boolean" Nullable="false"/>
        <Property Name="interval_seconds" Type="Edm.Int32" Nullable="true"/>
        <Property Name="last_run" Type="Edm.DateTimeOffset" Nullable="true"/>
        <Property Name="company_id" Type="Edm.Int32" Nullable="true"/>
        <Property Name="created_at" Type="Edm.DateTimeOffset" Nullable="false"/>
        <Property Name="updated_at" Type="Edm.DateTimeOffset" Nullable="false"/>
        <NavigationProperty Name="Readings" Type="Collection(NeemsAPI.Reading)"/>
        <NavigationProperty Name="Company" Type="NeemsAPI.Company" Nullable="true">
          <ReferentialConstraint Property="company_id" ReferencedProperty="id"/>
        </NavigationProperty>
      </EntityType>

      <!-- Device Entity Type -->
      <EntityType Name="Device">
        <Key>
          <PropertyRef Name="id"/>
        </Key>
        <Property Name="id" Type="Edm.Int32" Nullable="false"/>
        <Property Name="name" Type="Edm.String" Nullable="false"/>
        <Property Name="description" Type="Edm.String" Nullable="true"/>
        <Property Name="type_" Type="Edm.String" Nullable="false"/>
        <Property Name="model" Type="Edm.String" Nullable="false"/>
        <Property Name="serial" Type="Edm.String" Nullable="true"/>
        <Property Name="ip_address" Type="Edm.String" Nullable="true"/>
        <Property Name="install_date" Type="Edm.DateTimeOffset" Nullable="true"/>
        <Property Name="company_id" Type="Edm.Int32" Nullable="false"/>
        <Property Name="site_id" Type="Edm.Int32" Nullable="false"/>
        <NavigationProperty Name="Company" Type="NeemsAPI.Company" Nullable="false">
          <ReferentialConstraint Property="company_id" ReferencedProperty="id"/>
        </NavigationProperty>
        <NavigationProperty Name="Site" Type="NeemsAPI.Site" Nullable="false">
          <ReferentialConstraint Property="site_id" ReferencedProperty="id"/>
        </NavigationProperty>
      </EntityType>

      <!-- Reading Entity Type -->
      <EntityType Name="Reading">
        <Key>
          <PropertyRef Name="id"/>
        </Key>
        <Property Name="id" Type="Edm.Int32" Nullable="false"/>
        <Property Name="source_id" Type="Edm.Int32" Nullable="false"/>
        <Property Name="timestamp" Type="Edm.DateTimeOffset" Nullable="false"/>
        <Property Name="data" Type="Edm.String" Nullable="false"/>
        <Property Name="quality_flags" Type="Edm.Int32" Nullable="false"/>
        <NavigationProperty Name="DataSource" Type="NeemsAPI.DataSource" Nullable="false">
          <ReferentialConstraint Property="source_id" ReferencedProperty="id"/>
        </NavigationProperty>
      </EntityType>

      <!-- SchedulerScript Entity Type -->
      <EntityType Name="SchedulerScript">
        <Key>
          <PropertyRef Name="id"/>
        </Key>
        <Property Name="id" Type="Edm.Int32" Nullable="false"/>
        <Property Name="site_id" Type="Edm.Int32" Nullable="false"/>
        <Property Name="name" Type="Edm.String" Nullable="false"/>
        <Property Name="script_content" Type="Edm.String" Nullable="false"/>
        <Property Name="language" Type="Edm.String" Nullable="false"/>
        <Property Name="is_active" Type="Edm.Boolean" Nullable="false"/>
        <Property Name="version" Type="Edm.Int32" Nullable="false"/>
        <Property Name="created_at" Type="Edm.DateTimeOffset" Nullable="false"/>
        <Property Name="updated_at" Type="Edm.DateTimeOffset" Nullable="false"/>
        <NavigationProperty Name="Site" Type="NeemsAPI.Site" Nullable="false">
          <ReferentialConstraint Property="site_id" ReferencedProperty="id"/>
        </NavigationProperty>
      </EntityType>

      <!-- SchedulerOverride Entity Type -->
      <EntityType Name="SchedulerOverride">
        <Key>
          <PropertyRef Name="id"/>
        </Key>
        <Property Name="id" Type="Edm.Int32" Nullable="false"/>
        <Property Name="site_id" Type="Edm.Int32" Nullable="false"/>
        <Property Name="state" Type="Edm.String" Nullable="false"/>
        <Property Name="start_time" Type="Edm.DateTimeOffset" Nullable="false"/>
        <Property Name="end_time" Type="Edm.DateTimeOffset" Nullable="false"/>
        <Property Name="created_by" Type="Edm.Int32" Nullable="false"/>
        <Property Name="reason" Type="Edm.String" Nullable="true"/>
        <Property Name="is_active" Type="Edm.Boolean" Nullable="false"/>
        <Property Name="created_at" Type="Edm.DateTimeOffset" Nullable="false"/>
        <Property Name="updated_at" Type="Edm.DateTimeOffset" Nullable="false"/>
        <NavigationProperty Name="Site" Type="NeemsAPI.Site" Nullable="false">
          <ReferentialConstraint Property="site_id" ReferencedProperty="id"/>
        </NavigationProperty>
        <NavigationProperty Name="CreatedBy" Type="NeemsAPI.User" Nullable="false">
          <ReferentialConstraint Property="created_by" ReferencedProperty="id"/>
        </NavigationProperty>
      </EntityType>

      <!-- Entity Container -->
      <EntityContainer Name="DefaultContainer">
        <EntitySet Name="Users" EntityType="NeemsAPI.User">
          <NavigationPropertyBinding Path="Company" Target="Companies"/>
          <NavigationPropertyBinding Path="Roles" Target="Roles"/>
        </EntitySet>
        <EntitySet Name="Companies" EntityType="NeemsAPI.Company">
          <NavigationPropertyBinding Path="Users" Target="Users"/>
          <NavigationPropertyBinding Path="Sites" Target="Sites"/>
        </EntitySet>
        <EntitySet Name="Sites" EntityType="NeemsAPI.Site">
          <NavigationPropertyBinding Path="Company" Target="Companies"/>
          <NavigationPropertyBinding Path="SchedulerScripts" Target="SchedulerScripts"/>
          <NavigationPropertyBinding Path="SchedulerOverrides" Target="SchedulerOverrides"/>
        </EntitySet>
        <EntitySet Name="Devices" EntityType="NeemsAPI.Device">
          <NavigationPropertyBinding Path="Company" Target="Companies"/>
          <NavigationPropertyBinding Path="Site" Target="Sites"/>
        </EntitySet>
        <EntitySet Name="Roles" EntityType="NeemsAPI.Role">
          <NavigationPropertyBinding Path="Users" Target="Users"/>
        </EntitySet>
        <EntitySet Name="DataSources" EntityType="NeemsAPI.DataSource">
          <NavigationPropertyBinding Path="Readings" Target="Readings"/>
          <NavigationPropertyBinding Path="Company" Target="Companies"/>
        </EntitySet>
        <EntitySet Name="Readings" EntityType="NeemsAPI.Reading">
          <NavigationPropertyBinding Path="DataSource" Target="DataSources"/>
        </EntitySet>
        <EntitySet Name="SchedulerScripts" EntityType="NeemsAPI.SchedulerScript">
          <NavigationPropertyBinding Path="Site" Target="Sites"/>
        </EntitySet>
        <EntitySet Name="SchedulerOverrides" EntityType="NeemsAPI.SchedulerOverride">
          <NavigationPropertyBinding Path="Site" Target="Sites"/>
          <NavigationPropertyBinding Path="CreatedBy" Target="Users"/>
        </EntitySet>
      </EntityContainer>

    </Schema>
  </edmx:DataServices>
</edmx:Edmx>"#;

    RawXml(metadata.to_string())
}

/// Returns a vector of all OData-related routes.
pub fn routes() -> Vec<Route> {
    routes![service_document, metadata_document]
}