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

diesel::joinable!(sessions -> users (user_id));
diesel::joinable!(sites -> companies (company_id));
diesel::joinable!(user_roles -> roles (role_id));
diesel::joinable!(user_roles -> users (user_id));
diesel::joinable!(users -> companies (company_id));

diesel::allow_tables_to_appear_in_same_query!(
    companies,
    deleted_companies,
    deleted_users,
    entity_activity,
    roles,
    sessions,
    sites,
    user_roles,
    users,
);
