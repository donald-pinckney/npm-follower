// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "diff_type"))]
    pub struct DiffType;

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "internal_diff_log_version_state"))]
    pub struct InternalDiffLogVersionState;

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "package_state"))]
    pub struct PackageState;

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "package_state_enum"))]
    pub struct PackageStateEnum;

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "parsed_spec_struct"))]
    pub struct ParsedSpecStruct;

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "repo_info_struct"))]
    pub struct RepoInfoStruct;

    #[derive(diesel::sql_types::SqlType, diesel::query_builder::QueryId)]
    #[diesel(postgres_type(name = "semver_struct"))]
    pub struct SemverStruct;

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "version_state"))]
    pub struct VersionState;

    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "version_state_enum"))]
    pub struct VersionStateEnum;
}

diesel::table! {
    change_log (seq) {
        seq -> Int8,
        raw_json -> Jsonb,
        received_time -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::ParsedSpecStruct;

    dependencies (id) {
        id -> Int8,
        dst_package_name -> Text,
        dst_package_id_if_exists -> Nullable<Int8>,
        raw_spec -> Jsonb,
        spec -> ParsedSpecStruct,
        freq_count -> Int8,
        md5digest -> Text,
        md5digest_with_version -> Text,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::DiffType;
    use super::sql_types::SemverStruct;

    diff_log (id) {
        id -> Int8,
        seq -> Int8,
        package_name -> Text,
        dt -> DiffType,
        package_only_packument -> Nullable<Jsonb>,
        v -> Nullable<SemverStruct>,
        version_packument -> Nullable<Jsonb>,
    }
}

diesel::table! {
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

diesel::table! {
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

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::InternalDiffLogVersionState;

    internal_diff_log_state (package_name) {
        package_name -> Text,
        package_only_packument_hash -> Text,
        versions -> Array<InternalDiffLogVersionState>,
    }
}

diesel::table! {
    internal_state (key) {
        key -> Varchar,
        value -> Int8,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::PackageStateEnum;
    use super::sql_types::PackageState;

    packages (id) {
        id -> Int8,
        name -> Text,
        current_package_state_type -> PackageStateEnum,
        package_state_history -> Array<PackageState>,
        dist_tag_latest_version -> Nullable<Int8>,
        created -> Nullable<Timestamptz>,
        modified -> Nullable<Timestamptz>,
        other_dist_tags -> Nullable<Jsonb>,
        other_time_data -> Nullable<Jsonb>,
        unpublished_data -> Nullable<Jsonb>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::SemverStruct;
    use super::sql_types::VersionStateEnum;
    use super::sql_types::VersionState;
    use super::sql_types::RepoInfoStruct;

    versions (id) {
        id -> Int8,
        package_id -> Int8,
        semver -> SemverStruct,
        current_version_state_type -> VersionStateEnum,
        version_state_history -> Array<VersionState>,
        tarball_url -> Text,
        repository_raw -> Nullable<Jsonb>,
        repository_parsed -> Nullable<RepoInfoStruct>,
        created -> Timestamptz,
        extra_metadata -> Jsonb,
        prod_dependencies -> Array<Int8>,
        dev_dependencies -> Array<Int8>,
        peer_dependencies -> Array<Int8>,
        optional_dependencies -> Array<Int8>,
    }
}

diesel::joinable!(dependencies -> packages (dst_package_id_if_exists));
diesel::joinable!(diff_log -> change_log (seq));

diesel::allow_tables_to_appear_in_same_query!(
    change_log,
    dependencies,
    diff_log,
    download_tasks,
    downloaded_tarballs,
    internal_diff_log_state,
    internal_state,
    packages,
    versions,
);
