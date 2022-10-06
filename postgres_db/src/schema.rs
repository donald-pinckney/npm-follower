table! {
    use diesel::sql_types::*;
    use crate::custom_types::sql_type_names::*;

    change_log (seq) {
        seq -> Int8,
        raw_json -> Jsonb,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::custom_types::sql_type_names::*;

    dependencies (id) {
        id -> Int8,
        dst_package_name -> Text,
        dst_package_id_if_exists -> Nullable<Int8>,
        raw_spec -> Jsonb,
        spec -> Parsed_spec_struct,
        secret -> Bool,
        freq_count -> Int8,
        md5digest -> Text,
        md5digest_with_version -> Text,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::custom_types::sql_type_names::*;

    download_metrics (id) {
        id -> Int8,
        package_id -> Int8,
        download_counts -> Array<Download_count_struct>,
        latest_date -> Nullable<Date>,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::custom_types::sql_type_names::*;

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
        failed -> Nullable<Text>,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::custom_types::sql_type_names::*;

    downloaded_tarballs (tarball_url) {
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
    use diesel::sql_types::*;
    use crate::custom_types::sql_type_names::*;

    internal_state (key) {
        key -> Varchar,
        value -> Int8,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::custom_types::sql_type_names::*;

    packages (id) {
        id -> Int8,
        name -> Text,
        metadata -> Package_metadata_struct,
        secret -> Bool,
    }
}

table! {
    use diesel::sql_types::*;
    use crate::custom_types::sql_type_names::*;

    versions (id) {
        id -> Int8,
        package_id -> Int8,
        semver -> Semver_struct,
        tarball_url -> Text,
        repository_raw -> Nullable<Jsonb>,
        repository_parsed -> Nullable<Repo_info_struct>,
        created -> Timestamptz,
        deleted -> Bool,
        extra_metadata -> Jsonb,
        prod_dependencies -> Array<Int8>,
        dev_dependencies -> Array<Int8>,
        peer_dependencies -> Array<Int8>,
        optional_dependencies -> Array<Int8>,
        secret -> Bool,
    }
}

joinable!(dependencies -> packages (dst_package_id_if_exists));
joinable!(versions -> packages (package_id));

allow_tables_to_appear_in_same_query!(
    change_log,
    dependencies,
    download_metrics,
    download_tasks,
    downloaded_tarballs,
    internal_state,
    packages,
    versions,
);
