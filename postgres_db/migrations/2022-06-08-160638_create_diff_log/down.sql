DROP INDEX diff_log_pkg_idx;
DROP INDEX diff_log_seq_idx;

DROP TABLE diff_log, internal_diff_log_state CASCADE;

DROP TYPE       diff_type;
DROP TYPE       internal_diff_log_version_state;

DROP DOMAIN     semver;
DROP TYPE       semver_struct;
DROP DOMAIN     prerelease_tag;
DROP TYPE       prerelease_tag_struct;
DROP TYPE       prerelease_tag_type_enum;