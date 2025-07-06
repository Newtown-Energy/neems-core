// @generated automatically by Diesel CLI.

diesel::table! {
    clients (id) {
        id -> Integer,
        name -> Text,
    }
}

diesel::table! {
    institutions (id) {
        id -> Integer,
        name -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    sites (id) {
        id -> Integer,
        name -> Text,
        address -> Text,
        latitude -> Float,
        longitude -> Float,
    }
}

diesel::table! {
    users (id) {
        id -> Integer,
        username -> Text,
        email -> Text,
        password_hash -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        institution_id -> Integer,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    clients,
    institutions,
    sites,
    users,
);
