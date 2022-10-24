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
  (VALUE).bug IS NOT NULL AND
  (VALUE).prerelease IS NOT NULL AND
  (VALUE).build IS NOT NULL)
);


CREATE TYPE version_operator_enum AS ENUM ('*', '=', '>', '>=', '<', '<=');
CREATE TYPE version_comparator_struct AS (
  operator      version_operator_enum,
  semver        semver
);
CREATE DOMAIN version_comparator AS version_comparator_struct CHECK (
  (NOT VALUE IS NULL) AND (
  (VALUE).operator IS NOT NULL AND 
  (((VALUE).operator = '*' AND (VALUE).semver IS NULL) OR ((VALUE).operator <> '*' AND (VALUE).semver IS NOT NULL)))
);


CREATE DOMAIN constraint_conjuncts AS version_comparator[] CHECK (
  -- The list of conjuncts must be non-empty.
  -- An empty array has `array_length` NULL.
  array_length(VALUE, 1) IS NOT NULL
);

CREATE TYPE constraint_conjuncts_struct AS (
  conjuncts      constraint_conjuncts
);

CREATE DOMAIN constraint_disjuncts AS constraint_conjuncts_struct[] CHECK (
  -- The list of disjuncts must be non-empty.
  -- An empty array has `array_length` NULL.
  -- However, we explicitely allow the entire value to be NULL (otherwise that gets caught by the array_length check)
  VALUE IS NULL OR
  array_length(VALUE, 1) IS NOT NULL
);


CREATE TYPE dependency_type_enum AS ENUM (
  'range', 
  'tag', 
  'git', 
  'remote', 
  'alias', 
  'file', 
  'directory',
  'invalid'
);

CREATE TYPE alias_subdependency_type_enum AS ENUM (
  'range', 
  'tag'
);


CREATE TYPE parsed_spec_struct AS (
  dep_type                      dependency_type_enum,

  range_disjuncts               constraint_disjuncts,
  tag_name                      TEXT,
  git_spec                      TEXT,
  remote_url                    TEXT,
  alias_package_name            TEXT,
  alias_package_id_if_exists    BIGINT, -- REFERENCES packages(id), but we can't specify this
  alias_subdep_type             alias_subdependency_type_enum,
  alias_subdep_range_disjuncts  constraint_disjuncts,
  alias_subdep_tag_name         TEXT,
  file_path                     TEXT,
  directory_path                TEXT,
  invalid_message               TEXT
);

CREATE DOMAIN parsed_spec AS parsed_spec_struct
  CHECK(
    (VALUE).dep_type IS NOT NULL
  )

  CHECK(
    ((VALUE).dep_type = 'range' AND (VALUE).range_disjuncts IS NOT NULL) OR 
    ((VALUE).dep_type <> 'range' AND (VALUE).range_disjuncts IS NULL)
  )

  CHECK(
    ((VALUE).dep_type = 'tag' AND (VALUE).tag_name IS NOT NULL) OR 
    ((VALUE).dep_type <> 'tag' AND (VALUE).tag_name IS NULL)
  )

  CHECK(
    ((VALUE).dep_type = 'git' AND (VALUE).git_spec IS NOT NULL) OR 
    ((VALUE).dep_type <> 'git' AND (VALUE).git_spec IS NULL)
  )
  
  CHECK(
    ((VALUE).dep_type = 'remote' AND (VALUE).remote_url IS NOT NULL) OR 
    ((VALUE).dep_type <> 'remote' AND (VALUE).remote_url IS NULL)
  )

  CHECK(
    ((VALUE).dep_type = 'alias' AND 
      (VALUE).alias_package_name IS NOT NULL AND
      (VALUE).alias_subdep_type IS NOT NULL AND (
        ((VALUE).alias_subdep_type = 'range' AND (VALUE).alias_subdep_range_disjuncts IS NOT NULL AND (VALUE).alias_subdep_tag_name IS NULL) OR 
        ((VALUE).alias_subdep_type = 'tag' AND (VALUE).alias_subdep_range_disjuncts IS NULL AND (VALUE).alias_subdep_tag_name IS NOT NULL))) OR 
    ((VALUE).dep_type <> 'alias' AND 
      (VALUE).alias_package_name IS NULL AND
      (VALUE).alias_package_id_if_exists IS NULL AND
      (VALUE).alias_subdep_type IS NULL AND
      (VALUE).alias_subdep_range_disjuncts IS NULL AND
      (VALUE).alias_subdep_tag_name IS NULL)
  )

  CHECK(
    ((VALUE).dep_type = 'file' AND (VALUE).file_path IS NOT NULL) OR 
    ((VALUE).dep_type <> 'file' AND (VALUE).file_path IS NULL)
  )

  CHECK(
    ((VALUE).dep_type = 'directory' AND (VALUE).directory_path IS NOT NULL) OR 
    ((VALUE).dep_type <> 'directory' AND (VALUE).directory_path IS NULL)
  )

  CHECK(
    ((VALUE).dep_type = 'invalid' AND (VALUE).invalid_message IS NOT NULL) OR 
    ((VALUE).dep_type <> 'invalid' AND (VALUE).invalid_message IS NULL)
  );


CREATE TYPE package_state_enum AS ENUM (
  'normal', 
  'unpublished',
  'deleted'
);

CREATE TYPE package_metadata_struct AS (
  package_state               package_state_enum,
  dist_tag_latest_version     BIGINT, -- REFERENCES versions(id), but we can't specify this. May be null
  created                     TIMESTAMP WITH TIME ZONE,
  modified                    TIMESTAMP WITH TIME ZONE,
  other_dist_tags             JSONB,
  other_time_data             JSONB,
  unpublished_data            JSONB
);

CREATE DOMAIN package_metadata AS package_metadata_struct
  CHECK(
    (
      (VALUE).package_state = 'normal' AND
      -- (VALUE).dist_tag_latest_version can be null or non-null
      (VALUE).created IS NOT NULL AND 
      (VALUE).modified IS NOT NULL AND
      (VALUE).other_dist_tags IS NOT NULL AND
      (VALUE).other_time_data IS NULL AND
      (VALUE).unpublished_data IS NULL
    ) OR
    (
      (VALUE).package_state = 'unpublished' AND
      (VALUE).dist_tag_latest_version IS NULL AND
      (VALUE).created IS NOT NULL AND 
      (VALUE).modified IS NOT NULL AND
      (VALUE).other_dist_tags IS NULL AND
      (VALUE).other_time_data IS NOT NULL AND
      (VALUE).unpublished_data IS NOT NULL
    ) OR 
    (
      (VALUE).package_state = 'deleted' AND
      (VALUE).dist_tag_latest_version IS NULL AND
      (VALUE).created IS NULL AND 
      (VALUE).modified IS NULL AND
      (VALUE).other_dist_tags IS NULL AND
      (VALUE).other_time_data IS NULL AND
      (VALUE).unpublished_data IS NULL
    )
  )
  CHECK((VALUE).package_state IS NOT NULL);



CREATE TYPE vcs_type_enum AS ENUM (
  'git'
);

CREATE TYPE repo_host_enum AS ENUM (
  'github',
  'bitbucket',
  'gitlab',
  'gist',
  '3rdparty'
);

CREATE TYPE repo_info_struct AS (
  cloneable_repo_url            TEXT,
  cloneable_repo_dir            TEXT,
  vcs                           vcs_type_enum,
  host                          repo_host_enum,
  github_bitbucket_gitlab_user  TEXT,
  github_bitbucket_gitlab_repo  TEXT,
  gist_id                       TEXT
);

CREATE DOMAIN repo_info AS repo_info_struct
  CHECK(
    VALUE IS NULL OR (
      (VALUE).cloneable_repo_url IS NOT NULL AND 
      (VALUE).cloneable_repo_dir IS NOT NULL AND
      (VALUE).host IS NOT NULL AND
      (VALUE).vcs IS NOT NULL
    )
  )
  
  CHECK(
    VALUE IS NULL OR 
    (
      ((VALUE).host = 'github' OR (VALUE).host = 'bitbucket' OR (VALUE).host = 'gitlab') AND
      (VALUE).github_bitbucket_gitlab_user IS NOT NULL AND 
      (VALUE).github_bitbucket_gitlab_repo IS NOT NULL AND
      (VALUE).gist_id IS NULL AND
      (VALUE).vcs = 'git'
    ) OR
    (
      (VALUE).host = 'gist' AND
      (VALUE).github_bitbucket_gitlab_user IS NULL AND 
      (VALUE).github_bitbucket_gitlab_repo IS NULL AND
      (VALUE).gist_id IS NOT NULL AND
      (VALUE).vcs = 'git'
    ) OR
    (
      (VALUE).host = '3rdparty' AND
      (VALUE).github_bitbucket_gitlab_user IS NULL AND 
      (VALUE).github_bitbucket_gitlab_repo IS NULL AND
      (VALUE).gist_id IS NULL
    )
  );


------------------------------------------
-----                              -------
-----       TABLE DEFINITIONS      -------
-----                              -------
------------------------------------------

CREATE TYPE diff_type AS ENUM (
  'create_package',
  'update_package',
  -- 'set_package_latest_tag',
  'patch_package_references',
  'delete_package',
  'create_version',
  'update_version',
  'delete_version'
);

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
    (dt = 'delete_package'          AND package_only_packument IS NULL     AND v IS NULL     AND version_packument IS NULL) OR
    (dt = 'create_version'          AND package_only_packument IS NULL     AND v IS NOT NULL AND version_packument IS NOT NULL) OR
    (dt = 'update_version'          AND package_only_packument IS NULL     AND v IS NOT NULL AND version_packument IS NOT NULL) OR
    (dt = 'delete_version'          AND package_only_packument IS NULL     AND v IS NOT NULL AND version_packument IS NULL)
  )
);

CREATE TYPE internal_diff_log_version_state AS (
  v semver,
  version_packument_hash TEXT,
  deleted BOOLEAN
);

CREATE TABLE internal_diff_log_state (
  package_name TEXT PRIMARY KEY NOT NULL,
  package_only_packument_hash TEXT NOT NULL,
  deleted BOOLEAN NOT NULL,
  versions internal_diff_log_version_state[] NOT NULL
);


CREATE TABLE packages (
  id                          BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  name                        TEXT NOT NULL UNIQUE,
  metadata                    package_metadata NOT NULL,
  secret                      BOOLEAN NOT NULL
);




CREATE TABLE versions (
  id                      BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  package_id              BIGINT NOT NULL,
  semver                  semver NOT NULL,
  -- tarball_url references downloaded_tarballs(tarball_url), but note that that table allows 
  -- multiple downloads at different points in time, so the key is not unique.
  -- In addition, the tarball_url may not yet exist in downloaded_tarballs,
  -- if the tarball hasn't been downloaded yet!
  tarball_url             TEXT NOT NULL,
  repository_raw          JSONB,
  repository_parsed       repo_info,
  created                 TIMESTAMP WITH TIME ZONE NOT NULL,
  deleted                 BOOLEAN NOT NULL,
  extra_metadata JSONB    NOT NULL,

  -- These are all foreign keys to the dependencies(id),
  -- but we can't actually specify that:
  -- https://stackoverflow.com/a/50441059
  prod_dependencies       BIGINT[] NOT NULL,
  dev_dependencies        BIGINT[] NOT NULL,
  peer_dependencies       BIGINT[] NOT NULL,
  optional_dependencies   BIGINT[] NOT NULL,

  secret                  BOOLEAN NOT NULL,

  FOREIGN KEY(package_id) REFERENCES packages(id),
  UNIQUE(package_id, semver)
);

CREATE INDEX versions_package_id_idx ON versions (package_id);


CREATE TABLE dependencies (
  id                            BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,

  dst_package_name              TEXT NOT NULL,
  dst_package_id_if_exists      BIGINT,

  raw_spec                      JSONB NOT NULL,
  spec                          parsed_spec NOT NULL,

  secret                        BOOLEAN NOT NULL,

  freq_count                    BIGINT NOT NULL,
  md5digest                     TEXT NOT NULL, -- digest of only pkgname
  md5digest_with_version        TEXT NOT NULL, -- digest of "{pkgname}{raw_spec}"

  FOREIGN KEY(dst_package_id_if_exists) REFERENCES packages(id)
  -- We would like to specify this, but we can't
  -- FOREIGN KEY((spec).alias_package_id_if_exists) REFERENCES packages(id)
);

CREATE INDEX dependencies_alias_package_name_idx ON dependencies (((spec).alias_package_name)) WHERE (spec).dep_type = 'alias' AND (spec).alias_package_id_if_exists IS NULL;
CREATE INDEX dependencies_md5digest_idx ON dependencies (md5digest) WHERE dst_package_id_if_exists IS NULL;
CREATE INDEX dependencies_md5digest_with_version_idx ON dependencies (md5digest_with_version);


