//! TypeScript type generation module.
//!
//! This module exports TypeScript type definitions for all the structs
//! annotated with `#[ts(export)]`. When this file is compiled (typically
//! during testing), it generates .ts files in the specified output directory.

#[cfg(test)]
mod tests {
    use std::{env, path::Path};

    use ts_rs::TS;

    #[test]
    fn generate_typescript_types() {
        // Determine output directory in order of preference:
        // 1. Environment variable NEEMS_TS_OUTPUT_DIR
        // 2. ../../react/src/types/generated (if it exists)
        // 3. ../ts-bindings (fallback)

        let output_dir_str = if let Ok(env_dir) = env::var("NEEMS_TS_OUTPUT_DIR") {
            println!("Using TypeScript output directory from NEEMS_TS_OUTPUT_DIR: {}", env_dir);
            env_dir
        } else {
            let react_dir = "../../react/src/types/generated";
            let fallback_dir = "../ts-bindings";

            if Path::new(react_dir).parent().unwrap_or(Path::new("")).exists() {
                println!("Using React project directory: {}", react_dir);
                react_dir.to_string()
            } else {
                println!("Using fallback directory: {}", fallback_dir);
                fallback_dir.to_string()
            }
        };

        let output_dir = Path::new(&output_dir_str);

        // Create the output directory if it doesn't exist
        if !output_dir.exists() {
            std::fs::create_dir_all(output_dir).expect("Failed to create output directory");
        }

        // Clean up old TypeScript files to prevent orphaned definitions
        // This ensures that if a Rust type is removed or renamed, its .ts file is also
        // removed
        if output_dir.exists() {
            println!("Cleaning old TypeScript files from {:?}", output_dir);
            for entry in std::fs::read_dir(output_dir).expect("Failed to read output directory") {
                let entry = entry.expect("Failed to read directory entry");
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("ts") {
                    std::fs::remove_file(&path).expect(&format!("Failed to remove {:?}", path));
                }
            }
        }

        // Set the TS_RS_EXPORT_DIR environment variable
        unsafe {
            env::set_var("TS_RS_EXPORT_DIR", output_dir);
        }

        // Import all the types to trigger their generation
        use crate::{
            api::{
                company::ErrorResponse as CompanyErrorResponse,
                login::{ErrorResponse as LoginErrorResponse, LoginSuccessResponse},
                site::{CreateSiteRequest, ErrorResponse as SiteErrorResponse, UpdateSiteRequest},
                user::{
                    AddUserRoleRequest, CreateUserWithRolesRequest,
                    ErrorResponse as UserErrorResponse, RemoveUserRoleRequest, UpdateUserRequest,
                },
            },
            models::*,
        };

        // Export all the types
        User::export().expect("Failed to export User type");
        UserInput::export().expect("Failed to export UserInput type");
        UserWithRoles::export().expect("Failed to export UserWithRoles type");
        UserWithTimestamps::export().expect("Failed to export UserWithTimestamps type");
        UserWithRolesAndTimestamps::export()
            .expect("Failed to export UserWithRolesAndTimestamps type");

        Company::export().expect("Failed to export Company type");
        CompanyInput::export().expect("Failed to export CompanyInput type");
        CompanyWithTimestamps::export().expect("Failed to export CompanyWithTimestamps type");

        Site::export().expect("Failed to export Site type");

        Role::export().expect("Failed to export Role type");
        NewRole::export().expect("Failed to export NewRole type");

        // User API types
        UserErrorResponse::export().expect("Failed to export user::ErrorResponse type");
        CreateUserWithRolesRequest::export()
            .expect("Failed to export CreateUserWithRolesRequest type");
        AddUserRoleRequest::export().expect("Failed to export AddUserRoleRequest type");
        RemoveUserRoleRequest::export().expect("Failed to export RemoveUserRoleRequest type");
        UpdateUserRequest::export().expect("Failed to export UpdateUserRequest type");

        // Company API types
        CompanyErrorResponse::export().expect("Failed to export company::ErrorResponse type");

        // Site API types
        SiteErrorResponse::export().expect("Failed to export site::ErrorResponse type");
        CreateSiteRequest::export().expect("Failed to export CreateSiteRequest type");
        UpdateSiteRequest::export().expect("Failed to export UpdateSiteRequest type");

        // Login API types
        LoginErrorResponse::export().expect("Failed to export login::ErrorResponse type");
        LoginSuccessResponse::export().expect("Failed to export LoginSuccessResponse type");

        // Status API types
        use crate::api::status::HealthStatus;
        HealthStatus::export().expect("Failed to export HealthStatus type");

        // FixPhrase API types
        #[cfg(feature = "fixphrase")]
        use crate::api::fixphrase::FixPhraseResponse;
        #[cfg(feature = "fixphrase")]
        FixPhraseResponse::export().expect("Failed to export FixPhraseResponse type");

        // Role API types
        use crate::api::role::UpdateRoleRequest;
        UpdateRoleRequest::export().expect("Failed to export UpdateRoleRequest type");

        // Data API types
        use crate::api::data::{DataSourcesResponse, ReadingsQuery, ReadingsResponse};
        DataSourcesResponse::export().expect("Failed to export DataSourcesResponse type");
        ReadingsResponse::export().expect("Failed to export ReadingsResponse type");
        ReadingsQuery::export().expect("Failed to export ReadingsQuery type");

        // Neems-data model types
        neems_data::models::Source::export()
            .expect("Failed to export neems_data::models::Source type");
        neems_data::models::Reading::export()
            .expect("Failed to export neems_data::models::Reading type");

        println!("TypeScript types generated successfully in {:?}", output_dir);
    }
}
