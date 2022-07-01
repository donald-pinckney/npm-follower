-- This file should undo anything in `up.sql`

DROP INDEX dependencies_dst_package_name_idx;
DROP INDEX dependencies_alias_package_name_idx;
DROP TABLE packages, versions, dependencies CASCADE;

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
DROP DOMAIN     semver;
DROP TYPE       semver_struct;
DROP DOMAIN     prerelease_tag;
DROP TYPE       prerelease_tag_struct;
DROP TYPE       prerelease_tag_type_enum;