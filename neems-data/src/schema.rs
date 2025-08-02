// @generated automatically by Diesel CLI.

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
    sources (id) {
        id -> Nullable<Integer>,
        name -> Text,
        description -> Nullable<Text>,
        active -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::joinable!(readings -> sources (source_id));

diesel::allow_tables_to_appear_in_same_query!(readings, sources,);
