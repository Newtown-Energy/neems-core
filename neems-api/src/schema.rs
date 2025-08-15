// @generated automatically by Diesel CLI.

diesel::table! {
    companies (id) {
        id -> Integer,
        name -> Text,
    }
}

diesel::table! {
    deleted_companies (id) {
        id -> Integer,
        name -> Text,
        deleted_at -> Timestamp,
        deleted_by -> Nullable<Integer>,
    }
}

diesel::table! {
    deleted_users (id) {
        id -> Integer,
        email -> Text,
        password_hash -> Text,
        company_id -> Integer,
        totp_secret -> Nullable<Text>,
        deleted_at -> Timestamp,
        deleted_by -> Nullable<Integer>,
    }
}

diesel::table! {
    devices (id) {
        id -> Integer,
        name -> Text,
        description -> Nullable<Text>,
        #[sql_name = "type"]
        type_ -> Text,
        model -> Text,
        serial -> Nullable<Text>,
        ip_address -> Nullable<Text>,
        install_date -> Nullable<Timestamp>,
        company_id -> Integer,
        site_id -> Integer,
    }
}

diesel::table! {
    entity_activity (id) {
        id -> Integer,
        table_name -> Text,
        entity_id -> Integer,
        operation_type -> Text,
        timestamp -> Timestamp,
        user_id -> Nullable<Integer>,
    }
}

diesel::table! {
    roles (id) {
        id -> Integer,
        name -> Text,
        description -> Nullable<Text>,
    }
}

diesel::table! {
    scheduler_executions (id) {
        id -> Integer,
        site_id -> Integer,
        script_id -> Nullable<Integer>,
        override_id -> Nullable<Integer>,
        execution_time -> Timestamp,
        state_result -> Text,
        execution_duration_ms -> Nullable<Integer>,
        error_message -> Nullable<Text>,
    }
}

diesel::table! {
    scheduler_overrides (id) {
        id -> Integer,
        site_id -> Integer,
        state -> Text,
        start_time -> Timestamp,
        end_time -> Timestamp,
        created_by -> Integer,
        reason -> Nullable<Text>,
        is_active -> Bool,
    }
}

diesel::table! {
    scheduler_scripts (id) {
        id -> Integer,
        site_id -> Integer,
        name -> Text,
        script_content -> Text,
        language -> Text,
        is_active -> Bool,
        version -> Integer,
    }
}

diesel::table! {
    sessions (id) {
        id -> Text,
        user_id -> Integer,
        created_at -> Timestamp,
        expires_at -> Nullable<Timestamp>,
        revoked -> Bool,
    }
}

diesel::table! {
    sites (id) {
        id -> Integer,
        name -> Text,
        address -> Text,
        latitude -> Double,
        longitude -> Double,
        company_id -> Integer,
    }
}

diesel::table! {
    user_roles (user_id, role_id) {
        user_id -> Integer,
        role_id -> Integer,
    }
}

diesel::table! {
    users (id) {
        id -> Integer,
        email -> Text,
        password_hash -> Text,
        company_id -> Integer,
        totp_secret -> Nullable<Text>,
    }
}

diesel::joinable!(devices -> companies (company_id));
diesel::joinable!(devices -> sites (site_id));
diesel::joinable!(scheduler_executions -> scheduler_overrides (override_id));
diesel::joinable!(scheduler_executions -> scheduler_scripts (script_id));
diesel::joinable!(scheduler_executions -> sites (site_id));
diesel::joinable!(scheduler_overrides -> sites (site_id));
diesel::joinable!(scheduler_overrides -> users (created_by));
diesel::joinable!(scheduler_scripts -> sites (site_id));
diesel::joinable!(sessions -> users (user_id));
diesel::joinable!(sites -> companies (company_id));
diesel::joinable!(user_roles -> roles (role_id));
diesel::joinable!(user_roles -> users (user_id));
diesel::joinable!(users -> companies (company_id));

diesel::allow_tables_to_appear_in_same_query!(
    companies,
    deleted_companies,
    deleted_users,
    devices,
    entity_activity,
    roles,
    scheduler_executions,
    scheduler_overrides,
    scheduler_scripts,
    sessions,
    sites,
    user_roles,
    users,
);
