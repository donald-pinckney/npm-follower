CREATE SCHEMA IF NOT EXISTS metadata_analysis;

ALTER DEFAULT PRIVILEGES IN SCHEMA metadata_analysis
GRANT ALL ON TABLES TO pinckney,
    federico;
ALTER DEFAULT PRIVILEGES IN SCHEMA metadata_analysis
GRANT ALL ON SEQUENCES TO pinckney,
    federico;
ALTER DEFAULT PRIVILEGES IN SCHEMA metadata_analysis
GRANT ALL ON FUNCTIONS TO pinckney,
    federico;
ALTER DEFAULT PRIVILEGES IN SCHEMA metadata_analysis
GRANT ALL ON TYPES TO pinckney,
    federico;

ALTER DEFAULT PRIVILEGES IN SCHEMA metadata_analysis
GRANT SELECT ON TABLES TO data_analyzer;
ALTER DEFAULT PRIVILEGES IN SCHEMA metadata_analysis
GRANT USAGE,
    SELECT ON SEQUENCES TO data_analyzer;
ALTER DEFAULT PRIVILEGES IN SCHEMA metadata_analysis
GRANT EXECUTE ON FUNCTIONS TO data_analyzer;
ALTER DEFAULT PRIVILEGES IN SCHEMA metadata_analysis
GRANT USAGE ON TYPES TO data_analyzer;

ALTER DEFAULT PRIVILEGES IN SCHEMA PUBLIC
GRANT SELECT ON TABLES TO data_analyzer;
ALTER DEFAULT PRIVILEGES IN SCHEMA PUBLIC
GRANT USAGE,
    SELECT ON SEQUENCES TO data_analyzer;
ALTER DEFAULT PRIVILEGES IN SCHEMA PUBLIC
GRANT EXECUTE ON FUNCTIONS TO data_analyzer;
ALTER DEFAULT PRIVILEGES IN SCHEMA PUBLIC
GRANT USAGE ON TYPES TO data_analyzer;

ALTER DEFAULT PRIVILEGES
GRANT USAGE ON SCHEMAS TO data_analyzer;

GRANT ALL ON SCHEMA metadata_analysis TO federico;
GRANT ALL ON SCHEMA metadata_analysis TO pinckney;
GRANT USAGE ON SCHEMA metadata_analysis TO data_analyzer;



CREATE TYPE metadata_analysis.update_type AS ENUM ('zero_to_something', 'bug', 'minor', 'major');

CREATE OR REPLACE FUNCTION metadata_analysis.determine_update_type(semver, semver) RETURNS metadata_analysis.update_type AS $$ -- $1 = from
    -- $2 = to
SELECT CASE
        WHEN ($1) >= ($2) THEN NULL
        WHEN ($1).prerelease IS NOT NULL
        OR ($1).build IS NOT NULL
        OR ($2).prerelease IS NOT NULL
        OR ($2).build IS NOT NULL THEN NULL
        WHEN ($1).major = 0
        AND ($1).minor = 0
        AND ($1).bug = 0 THEN 'zero_to_something'::metadata_analysis.update_type
        WHEN ($1).major = ($2).major
        AND ($1).minor = ($2).minor THEN 'bug'::metadata_analysis.update_type
        WHEN ($1).major = ($2).major THEN 'minor'::metadata_analysis.update_type
        ELSE 'major'::metadata_analysis.update_type
    END $$ LANGUAGE SQL IMMUTABLE;