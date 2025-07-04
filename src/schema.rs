// @generated automatically by Diesel CLI.

diesel::table! {
    clients (id) {
        id -> Int4,
        name -> VarChar,
    }
}

diesel::table! {
    sites (id) {
        id -> Int4,
        name -> VarChar,
	address -> VarChar,
	latitude -> Float8,
	longitude -> Float8,
    }
}
