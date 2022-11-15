DROP INDEX dependencies_alias_package_name_idx;
DROP INDEX dependencies_md5digest_idx;
DROP INDEX dependencies_md5digest_with_version_idx;
DROP INDEX versions_package_id_idx;

ALTER TABLE packages DROP CONSTRAINT fkey_packages_dist_tag_latest_version;
DROP TABLE packages, versions, dependencies CASCADE;


DROP DOMAIN     package_state;
DROP TYPE       package_state_struct;
DROP DOMAIN     version_state;
DROP TYPE       version_state_struct;
DROP TYPE       version_state_enum;
DROP DOMAIN     repo_info;
DROP TYPE       repo_info_struct;
DROP TYPE       repo_host_enum;
DROP TYPE       vcs_type_enum;
DROP DOMAIN     package_metadata;
DROP TYPE       package_metadata_struct;
DROP TYPE       package_state_enum;
DROP DOMAIN     parsed_spec;
DROP TYPE       parsed_spec_struct;
DROP TYPE       dependency_type_enum;
DROP TYPE       alias_subdependency_type_enum;
DROP DOMAIN     constraint_disjuncts;
DROP TYPE       constraint_conjuncts_struct;
DROP DOMAIN     constraint_conjuncts;
DROP DOMAIN     version_comparator;
DROP TYPE       version_comparator_struct;
DROP TYPE       version_operator_enum;