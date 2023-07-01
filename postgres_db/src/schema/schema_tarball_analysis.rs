// @generated automatically by Diesel CLI.

pub mod tarball_analysis {
    pub mod sql_types {
        #[derive(diesel::sql_types::SqlType)]
        #[diesel(postgres_type(name = "update_type", schema = "metadata_analysis"))]
        pub struct UpdateType;
    }

    diesel::table! {
        tarball_analysis.diff_analysis (from_id, to_id) {
            from_id -> Int8,
            to_id -> Int8,
            job_result -> Jsonb,
        }
    }

    diesel::table! {
        tarball_analysis.diff_changed_files (from_id, to_id) {
            from_id -> Int8,
            to_id -> Int8,
            did_change_types -> Bool,
            did_change_code -> Bool,
        }
    }

    diesel::table! {
        tarball_analysis.diff_ext_count (ext) {
            ext -> Text,
            count -> Int8,
        }
    }

    diesel::table! {
        tarball_analysis.diff_num_files (from_id, to_id) {
            from_id -> Int8,
            to_id -> Int8,
            num_files_added -> Int8,
            num_files_modified -> Int8,
            num_files_deleted -> Int8,
        }
    }

    diesel::table! {
        tarball_analysis.diff_num_lines (from_id, to_id) {
            from_id -> Int8,
            to_id -> Int8,
            num_lines_added -> Int8,
            num_lines_deleted -> Int8,
        }
    }

    diesel::table! {
        tarball_analysis.size_analysis_tarball (tarball_url) {
            tarball_url -> Text,
            total_files -> Nullable<Int8>,
            total_size -> Nullable<Int8>,
            total_size_code -> Nullable<Int8>,
        }
    }

    diesel::table! {
        use diesel::sql_types::*;
        use super::sql_types::UpdateType;

        tarball_analysis.what_did_updates_change (from_id, to_id) {
            from_id -> Int8,
            to_id -> Int8,
            package_id -> Nullable<Int8>,
            ty -> Nullable<UpdateType>,
            from_created -> Nullable<Timestamptz>,
            to_created -> Nullable<Timestamptz>,
            did_intro_vuln -> Nullable<Bool>,
            did_patch_vuln -> Nullable<Bool>,
            did_change_types -> Nullable<Bool>,
            did_change_code -> Nullable<Bool>,
            did_add_dep -> Nullable<Bool>,
            did_remove_dep -> Nullable<Bool>,
            did_modify_dep_constraint -> Nullable<Bool>,
            did_change_json_scripts -> Nullable<Bool>,
        }
    }

    diesel::allow_tables_to_appear_in_same_query!(
        diff_analysis,
        diff_changed_files,
        diff_ext_count,
        diff_num_files,
        diff_num_lines,
        size_analysis_tarball,
        what_did_updates_change,
    );
}
