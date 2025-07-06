// We have a set of clients.
diesel::table! {
    clients (id) {
        id -> Int4,
        name -> VarChar,
    }
}

// Each client can have multiple sites.  Each site belongs to one client.
iesel::table! {
    sites (id) {
        id -> Int4,
        name -> VarChar,
        address -> VarChar,
        latitude -> Float8,
        longitude -> Float8,
        client_id -> Int4, // Foreign key for the client
    }
}

diesel::joinable!(sites -> clients (client_id));

diesel::allow_tables_to_appear_in_same_query!(
    clients,
    sites,
);


