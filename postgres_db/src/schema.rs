table! {
    change_log (seq) {
        seq -> Int8,
        raw_json -> Jsonb,
    }
}

table! {
    download_tasks (package, version) {
        package -> Varchar,
        version -> Varchar,
        url -> Varchar,
        change_seq -> Int8,
        shasum -> Nullable<Text>,
        unpacked_size -> Nullable<Int8>,
        file_count -> Nullable<Int4>,
        integrity -> Nullable<Text>,
        signature0_sig -> Nullable<Text>,
        signature0_keyid -> Nullable<Text>,
        npm_signature -> Nullable<Text>,
        queue_time -> Timestamptz,
        num_failures -> Int4,
        last_failure -> Nullable<Timestamptz>,
        success -> Bool,
    }
}

table! {
    internal_state (key) {
        key -> Varchar,
        value -> Int8,
    }
}

joinable!(download_tasks -> change_log (change_seq));

allow_tables_to_appear_in_same_query!(
    change_log,
    download_tasks,
    internal_state,
);
