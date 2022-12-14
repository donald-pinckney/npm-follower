------------------------------------------
-----                              -------
-----    CUSTOM TYPE DEFINITIONS   -------
-----                              -------
------------------------------------------

CREATE TYPE prerelease_tag_type_enum AS ENUM ('string', 'int');
CREATE TYPE prerelease_tag_struct AS (
  tag_type          prerelease_tag_type_enum,
  string_case       TEXT,
  int_case          BIGINT
); 
CREATE DOMAIN prerelease_tag AS prerelease_tag_struct CHECK (
  (NOT VALUE IS NULL) AND
  (((VALUE).tag_type = 'string' AND (VALUE).string_case IS NOT NULL AND (VALUE).int_case IS NULL) 
    OR 
  ((VALUE).tag_type = 'int' AND (VALUE).string_case IS NULL AND (VALUE).int_case IS NOT NULL))
);


CREATE TYPE semver_struct AS (
  major                   BIGINT,
  minor                   BIGINT,
  bug                     BIGINT,
  prerelease              prerelease_tag[],
  build                   TEXT[]
);
CREATE DOMAIN semver AS semver_struct CHECK (
  VALUE IS NULL OR (
    (VALUE).major IS NOT NULL AND 
    (VALUE).minor IS NOT NULL AND
    (VALUE).bug IS NOT NULL --AND
    -- (VALUE).prerelease IS NOT NULL AND
    -- (VALUE).build IS NOT NULL
  )
);


CREATE TYPE diff_type AS ENUM (
  'create_package',
  'update_package',
  'patch_package_references',
  'delete_package',
  'create_version',
  'update_version',
  'delete_version'
);

CREATE TYPE internal_diff_log_version_state AS (
  v semver,
  version_packument_hash TEXT,
  deleted BOOLEAN
);

------------------------------------------
-----                              -------
-----       TABLE DEFINITIONS      -------
-----                              -------
------------------------------------------

CREATE TABLE diff_log (
  id BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  seq BIGINT NOT NULL,
  package_name TEXT NOT NULL,
  dt diff_type NOT NULL,
  package_only_packument JSONB,
  v semver,
  version_packument JSONB,


  FOREIGN KEY(seq) REFERENCES change_log(seq),
  CONSTRAINT version_diff_valid CHECK (
    (dt = 'create_package'          AND package_only_packument IS NOT NULL AND v IS NULL     AND version_packument IS NULL) OR
    (dt = 'update_package'          AND package_only_packument IS NOT NULL AND v IS NULL     AND version_packument IS NULL) OR
    -- (dt = 'set_package_latest_tag'  AND package_only_packument IS NULL     AND v IS NULL AND version_packument IS NULL) OR
    (dt = 'patch_package_references'  AND package_only_packument IS NULL     AND v IS NULL AND version_packument IS NULL) OR
    -- (dt = 'delete_package'          AND package_only_packument IS NULL     AND v IS NULL     AND version_packument IS NULL) OR
    (dt = 'create_version'          AND package_only_packument IS NULL     AND (NOT v IS NULL) AND version_packument IS NOT NULL) OR
    (dt = 'update_version'          AND package_only_packument IS NULL     AND (NOT v IS NULL) AND version_packument IS NOT NULL) OR
    (dt = 'delete_version'          AND package_only_packument IS NULL     AND (NOT v IS NULL) AND version_packument IS NULL)
  )
);

CREATE INDEX diff_log_pkg_idx ON diff_log (package_name);
CREATE INDEX diff_log_seq_idx ON diff_log (seq);

CREATE TABLE internal_diff_log_state (
  package_name TEXT PRIMARY KEY NOT NULL,
  package_only_packument_hash TEXT NOT NULL,
  -- deleted BOOLEAN NOT NULL,
  versions internal_diff_log_version_state[] NOT NULL
);

