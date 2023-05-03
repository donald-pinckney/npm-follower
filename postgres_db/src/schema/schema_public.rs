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
        prod_freq_count -> Int8,
        dev_freq_count -> Int8,
        peer_freq_count -> Int8,
        optional_freq_count -> Int8,
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
        tgz_local_path -> Nullable<Text>,
        blob_storage_key -> Nullable<Text>,
    }
}

diesel::table! {
    ghsa (id) {
        id -> Text,
        severity -> Text,
        description -> Text,
        summary -> Text,
        withdrawn_at -> Nullable<Timestamptz>,
        published_at -> Timestamptz,
        updated_at -> Timestamptz,
        refs -> Array<Text>,
        cvss_score -> Nullable<Float4>,
        cvss_vector -> Nullable<Text>,
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
        int_value -> Nullable<Int8>,
        string_value -> Nullable<Text>,
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

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::SemverStruct;

    vulnerabilities (id) {
        id -> Int8,
        ghsa_id -> Text,
        package_name -> Text,
        vulnerable_version_lower_bound -> Nullable<SemverStruct>,
        vulnerable_version_lower_bound_inclusive -> Bool,
        vulnerable_version_upper_bound -> Nullable<SemverStruct>,
        vulnerable_version_upper_bound_inclusive -> Bool,
        first_patched_version -> Nullable<SemverStruct>,
    }
}

diesel::joinable!(dependencies -> packages (dst_package_id_if_exists));
diesel::joinable!(diff_log -> change_log (seq));
diesel::joinable!(vulnerabilities -> ghsa (ghsa_id));

diesel::allow_tables_to_appear_in_same_query!(
    change_log,
    dependencies,
    diff_log,
    download_tasks,
    downloaded_tarballs,
    ghsa,
    internal_diff_log_state,
    internal_state,
    packages,
    versions,
    vulnerabilities,
);
