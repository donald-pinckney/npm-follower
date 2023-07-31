------------------------------------------
-----                              -------
-----    CUSTOM TYPE DEFINITIONS   -------
-----                              -------
------------------------------------------


CREATE TYPE version_operator_enum AS ENUM ('*', '=', '>', '>=', '<', '<=');
CREATE TYPE version_comparator_struct AS (
  operator      version_operator_enum,
  semver        semver
);
CREATE DOMAIN version_comparator AS version_comparator_struct CHECK (
  (NOT VALUE IS NULL) AND (
  (VALUE).operator IS NOT NULL AND 
  (((VALUE).operator = '*' AND (VALUE).semver IS NULL) OR ((VALUE).operator <> '*' AND NOT (VALUE).semver IS NULL)))
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

-- TODO: dead code, remove
CREATE TYPE package_metadata_struct AS (
  package_state               package_state_enum,
  dist_tag_latest_version     BIGINT, -- REFERENCES versions(id), but we can't specify this. May be null
  created                     TIMESTAMP WITH TIME ZONE,
  modified                    TIMESTAMP WITH TIME ZONE,
  other_dist_tags             JSONB,
  other_time_data             JSONB,
  unpublished_data            JSONB
);

-- TODO: dead code, remove
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





CREATE TYPE package_state_struct AS (
  package_state_type          package_state_enum,
  seq                         BIGINT, -- REFERENCES change_log(seq), but we can't specify this.
  diff_entry_id               BIGINT, -- REFERENCES diff_log(id), but we can't specify this.
  estimated_time              TIMESTAMP WITH TIME ZONE
);


CREATE DOMAIN package_state AS package_state_struct CHECK (
  (VALUE).package_state_type IS NOT NULL AND 
  (VALUE).seq IS NOT NULL AND 
  (VALUE).diff_entry_id IS NOT NULL --AND 
  -- (VALUE).estimated_time IS NOT NULL
);


CREATE TYPE version_state_enum AS ENUM (
  'normal', 
  'unpublished',
  'deleted'
);

CREATE TYPE version_state_struct AS (
  version_state_type          version_state_enum,
  seq                         BIGINT, -- REFERENCES change_log(seq), but we can't specify this.
  diff_entry_id               BIGINT, -- REFERENCES diff_log(id), but we can't specify this.
  estimated_time              TIMESTAMP WITH TIME ZONE
);


CREATE DOMAIN version_state AS version_state_struct CHECK (
  (VALUE).version_state_type IS NOT NULL AND 
  (VALUE).seq IS NOT NULL AND 
  (VALUE).diff_entry_id IS NOT NULL --AND 
  -- (VALUE).estimated_time IS NOT NULL
);





------------------------------------------
-----                              -------
-----       TABLE DEFINITIONS      -------
-----                              -------
------------------------------------------


CREATE TABLE packages (
  id                          BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  name                        TEXT NOT NULL UNIQUE,
  
  current_package_state_type  package_state_enum NOT NULL,
  package_state_history       package_state[] NOT NULL,

  dist_tag_latest_version     BIGINT, -- REFERENCES versions(id) must be specified later
  created                     TIMESTAMP WITH TIME ZONE,
  modified                    TIMESTAMP WITH TIME ZONE,
  other_dist_tags             JSONB,
  other_time_data             JSONB,
  unpublished_data            JSONB,

  CONSTRAINT check_current_package_state_matches CHECK (
    array_length(package_state_history, 1) IS NOT NULL AND
    package_state_history[array_upper(package_state_history, 1)].package_state_type = current_package_state_type
  ),

  CONSTRAINT check_package_data_filled CHECK (
    (
      current_package_state_type = 'normal' AND
      -- dist_tag_latest_version IS NOT NULL AND -- this might not be right???
      created IS NOT NULL AND 
      modified IS NOT NULL AND
      other_dist_tags IS NOT NULL AND
      -- other_time_data IS NULL AND
      unpublished_data IS NULL
    ) OR
    (
      current_package_state_type = 'unpublished' AND
      --dist_tag_latest_version IS NULL AND     ;;  could be NULL or not, if we had data previously
      created IS NOT NULL AND 
      modified IS NOT NULL AND
      -- other_dist_tags IS NULL AND            ;;  could be NULL or not, if we had data previously
      -- other_time_data IS NOT NULL AND -- ???
      unpublished_data IS NOT NULL
    ) OR 
    (
      current_package_state_type = 'deleted' --AND
      --dist_tag_latest_version IS NULL AND     ;;  could be NULL or not, if we had data previously
      --created IS NULL AND                     ;;  could be NULL or not, if we had data previously
      --modified IS NULL AND                    ;;  could be NULL or not, if we had data previously
      -- other_dist_tags IS NULL AND            ;;  could be NULL or not, if we had data previously
      -- other_time_data IS NULL AND            ;;  could be NULL or not, if we had data previously
      -- unpublished_data IS NULL               ;;  could be NULL or not, if we had data previously
    )
  )
);




CREATE TABLE versions (
  id                      BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  package_id              BIGINT NOT NULL,
  semver                  semver NOT NULL,

  current_version_state_type  version_state_enum NOT NULL,
  version_state_history       version_state[] NOT NULL,

  -- tarball_url references downloaded_tarballs(tarball_url), but note that that table allows 
  -- multiple downloads at different points in time, so the key is not unique.
  -- In addition, the tarball_url may not yet exist in downloaded_tarballs,
  -- if the tarball hasn't been downloaded yet!
  tarball_url             TEXT NOT NULL,
  repository_raw          JSONB,
  repository_parsed       repo_info,
  created                 TIMESTAMP WITH TIME ZONE NOT NULL,
  extra_metadata JSONB    NOT NULL,

  -- These are all foreign keys to the dependencies(id),
  -- but we can't actually specify that:
  -- https://stackoverflow.com/a/50441059
  prod_dependencies       BIGINT[] NOT NULL,
  dev_dependencies        BIGINT[] NOT NULL,
  peer_dependencies       BIGINT[] NOT NULL,
  optional_dependencies   BIGINT[] NOT NULL,

  FOREIGN KEY(package_id) REFERENCES packages(id), -- move this, not working with diesel?
  UNIQUE(package_id, semver),

  CONSTRAINT check_current_version_state_matches CHECK (
    array_length(version_state_history, 1) IS NOT NULL AND
    version_state_history[array_upper(version_state_history, 1)].version_state_type = current_version_state_type
  )
);

ALTER TABLE packages ADD CONSTRAINT fkey_packages_dist_tag_latest_version FOREIGN KEY (dist_tag_latest_version) REFERENCES versions(id);
CREATE INDEX versions_package_id_idx ON versions (package_id);
-- CREATE INDEX versions_idx_semver_non_beta ON versions ((semver).prerelease is null and (semver).build is null);
CREATE INDEX versions_idx_created ON versions (created);


CREATE TABLE dependencies (
  id                            BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,

  dst_package_name              TEXT NOT NULL,
  dst_package_id_if_exists      BIGINT,

  raw_spec                      JSONB NOT NULL,
  spec                          parsed_spec NOT NULL,

  prod_freq_count               BIGINT NOT NULL,
  dev_freq_count                BIGINT NOT NULL,
  peer_freq_count               BIGINT NOT NULL,
  optional_freq_count           BIGINT NOT NULL,

  md5digest                     TEXT NOT NULL, -- digest of only pkgname
  md5digest_with_version        TEXT NOT NULL, -- digest of "{pkgname}{raw_spec}"

  FOREIGN KEY(dst_package_id_if_exists) REFERENCES packages(id)
  -- We would like to specify this, but we can't
  -- FOREIGN KEY((spec).alias_package_id_if_exists) REFERENCES packages(id)
);

CREATE INDEX dependencies_dst_package_id_if_exists_idx ON dependencies (dst_package_id_if_exists);
-- TODO: delete this index?
-- CREATE INDEX dependencies_alias_package_name_idx ON dependencies (((spec).alias_package_name)) WHERE (spec).dep_type = 'alias' AND (spec).alias_package_id_if_exists IS NULL;
CREATE INDEX dependencies_md5digest_idx ON dependencies (md5digest) WHERE dst_package_id_if_exists IS NULL;
-- CREATE INDEX dependencies_md5digest_with_version_idx ON dependencies (md5digest_with_version);
ALTER TABLE dependencies ADD CONSTRAINT dependencies_md5digest_with_version_unique UNIQUE (md5digest_with_version);
