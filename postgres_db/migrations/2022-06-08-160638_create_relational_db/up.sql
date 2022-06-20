------------------------------------------
-----                              -------
-----    CUSTOM TYPE DEFINITIONS   -------
-----                              -------
------------------------------------------

CREATE TYPE prerelease_tag_type_enum AS ENUM ('string', 'int');
CREATE TYPE prerelease_tag_struct AS (
  tag_type          prerelease_tag_type_enum,
  string_case       TEXT,
  int_case          INTEGER
); 
CREATE DOMAIN prerelease_tag AS prerelease_tag_struct CHECK (
  (NOT VALUE IS NULL) AND
  (((VALUE).tag_type = 'string' AND (VALUE).string_case IS NOT NULL AND (VALUE).int_case IS NULL) 
    OR 
  ((VALUE).tag_type = 'int' AND (VALUE).string_case IS NULL AND (VALUE).int_case IS NOT NULL))
);


CREATE TYPE semver_struct AS (
  major                   INTEGER,
  minor                   INTEGER,
  bug                     INTEGER,
  prerelease              prerelease_tag[],
  build                   prerelease_tag[]
);
CREATE DOMAIN semver AS semver_struct CHECK (
  VALUE IS NULL OR (
  (VALUE).major IS NOT NULL AND 
  (VALUE).minor IS NOT NULL AND
  (VALUE).bug IS NOT NULL AND
  (VALUE).prerelease IS NOT NULL AND
  (VALUE).build IS NOT NULL)
);


CREATE TYPE repository_type_enum AS ENUM ('git');
CREATE TYPE repository_struct AS (
  repo_type               repository_type_enum,
  url                     TEXT
);
CREATE DOMAIN repository AS repository_struct CHECK (
  VALUE IS NULL OR 
  ((VALUE).repo_type IS NOT NULL AND 
  (VALUE).url IS NOT NULL)
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



------------------------------------------
-----                              -------
-----       TABLE DEFINITIONS      -------
-----                              -------
------------------------------------------



CREATE TABLE packages (
  id                          BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  name                        TEXT NOT NULL UNIQUE,
  dist_tag_latest_version     BIGINT NOT NULL, -- not sure if NOT NULL is right here
  created                     TIMESTAMP WITH TIME ZONE NOT NULL,
  modified                    TIMESTAMP WITH TIME ZONE NOT NULL,
  deleted                     BOOLEAN NOT NULL,
  other_dist_tags             JSONB NOT NULL
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
  description             TEXT,
  repository              repository,
  created                 TIMESTAMP WITH TIME ZONE NOT NULL,
  deleted                     BOOLEAN NOT NULL,
  extra_metadata JSONB    NOT NULL,

  -- These are all foreign keys to the dependencies(id),
  -- but we can't actually specify that:
  -- https://stackoverflow.com/a/50441059
  prod_dependencies       BIGINT[] NOT NULL,
  dev_dependencies        BIGINT[] NOT NULL,
  peer_dependencies       BIGINT[] NOT NULL,
  optional_dependencies   BIGINT[] NOT NULL,

  FOREIGN KEY(package_id) REFERENCES packages(id)
);

ALTER TABLE packages ADD FOREIGN KEY(dist_tag_latest_version) REFERENCES versions(id);



CREATE TABLE dependencies (
  id                          BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,

  dst_package_name            TEXT NOT NULL,
  dst_package_id_if_exists    BIGINT,

  version_constraint_raw      TEXT NOT NULL,
  disjuncts_conjuncts         version_comparator[][] NOT NULL,
  
  FOREIGN KEY(dst_package_id_if_exists) REFERENCES packages(id)
);