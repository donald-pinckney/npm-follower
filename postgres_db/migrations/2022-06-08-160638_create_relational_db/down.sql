-- This file should undo anything in `up.sql`

ALTER TABLE versions
DROP CONSTRAINT versions_package_id_fkey;

ALTER TABLE packages
DROP CONSTRAINT packages_dist_tag_latest_version_fkey;

ALTER TABLE dependencies
DROP CONSTRAINT dependencies_dst_package_id_if_exists_fkey;


DROP TABLE packages;
DROP TABLE versions;
DROP TABLE dependencies;


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
DROP DOMAIN     repository;
DROP TYPE       repository_struct;
DROP TYPE       repository_type_enum;