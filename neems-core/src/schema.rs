// @generated automatically by Diesel CLI.

diesel::table! {
    institutions (id) {
        id -> Nullable<Integer>,
        name -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    roles (id) {
        id -> Nullable<Integer>,
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
        id -> Nullable<Integer>,
        name -> Text,
        address -> Text,
        latitude -> Double,
        longitude -> Double,
        institution_id -> Integer,
        created_at -> Timestamp,
        updated_at -> Timestamp,
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
        id -> Nullable<Integer>,
        email -> Text,
        password_hash -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        institution_id -> Integer,
        totp_secret -> Text,
    }
}

diesel::joinable!(sessions -> users (user_id));
diesel::joinable!(sites -> institutions (institution_id));
diesel::joinable!(user_roles -> roles (role_id));
diesel::joinable!(user_roles -> users (user_id));
diesel::joinable!(users -> institutions (institution_id));

diesel::allow_tables_to_appear_in_same_query!(
    institutions,
    roles,
    sessions,
    sites,
    user_roles,
    users,
);
