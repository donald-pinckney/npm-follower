table! {
    change_log (seq) {
        seq -> Int8,
        raw_json -> Jsonb,
    }
}

table! {
    dependencies (id) {
        id -> Int8,
        dst_package_name -> Text,
        dst_package_id_if_exists -> Nullable<Int8>,
        version_constraint_raw -> Text,
        disjuncts_conjuncts -> Array<Version_comparator>,
    }
}

table! {
    download_tasks (url) {
        url -> Varchar,
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
    downloaded_tarballs (tarball_url, downloaded_at) {
        tarball_url -> Text,
        downloaded_at -> Timestamptz,
        shasum -> Nullable<Text>,
        unpacked_size -> Nullable<Int8>,
        file_count -> Nullable<Int4>,
        integrity -> Nullable<Text>,
        signature0_sig -> Nullable<Text>,
        signature0_keyid -> Nullable<Text>,
        npm_signature -> Nullable<Text>,
        tgz_local_path -> Text,
    }
}

table! {
    internal_state (key) {
        key -> Varchar,
        value -> Int8,
    }
}

table! {
    packages (id) {
        id -> Int8,
        name -> Text,
        dist_tag_latest_version -> Int8,
        created -> Timestamptz,
        modified -> Timestamptz,
        other_dist_tags -> Jsonb,
    }
}

table! {
    versions (id) {
        id -> Int8,
        package_id -> Int8,
        semver -> Semver_struct,
        tarball_url -> Text,
        description -> Nullable<Text>,
        repository -> Nullable<Repository_struct>,
        created -> Timestamptz,
        extra_metadata -> Jsonb,
        prod_dependencies -> Array<Int8>,
        dev_dependencies -> Array<Int8>,
        peer_dependencies -> Array<Int8>,
        optional_dependencies -> Array<Int8>,
    }
}

joinable!(dependencies -> packages (dst_package_id_if_exists));

allow_tables_to_appear_in_same_query!(
    change_log,
    dependencies,
    download_tasks,
    downloaded_tarballs,
    internal_state,
    packages,
    versions,
);
