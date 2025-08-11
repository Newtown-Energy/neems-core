// @generated automatically by Diesel CLI.

diesel::table! {
    companies (id) {
        id -> Integer,
        name -> Text,
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
    readings (id) {
        id -> Nullable<Integer>,
        source_id -> Integer,
        timestamp -> Timestamp,
        data -> Text,
        quality_flags -> Integer,
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
    sources (id) {
        id -> Nullable<Integer>,
        name -> Text,
        description -> Nullable<Text>,
        active -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        interval_seconds -> Integer,
        last_run -> Nullable<Timestamp>,
        test_type -> Nullable<Text>,
        arguments -> Nullable<Text>,
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

diesel::joinable!(entity_activity -> users (user_id));
diesel::joinable!(readings -> sources (source_id));
diesel::joinable!(sessions -> users (user_id));
diesel::joinable!(sites -> companies (company_id));
diesel::joinable!(user_roles -> roles (role_id));
diesel::joinable!(user_roles -> users (user_id));
diesel::joinable!(users -> companies (company_id));

diesel::allow_tables_to_appear_in_same_query!(
    companies,
    entity_activity,
    readings,
    roles,
    sessions,
    sites,
    sources,
    user_roles,
    users,
);
