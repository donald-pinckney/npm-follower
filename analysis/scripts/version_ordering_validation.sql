CREATE OR REPLACE FUNCTION analysis.base_compatible_semver(semver) RETURNS semver AS $$
SELECT CASE
    WHEN ($1).prerelease IS NOT NULL
    OR ($1).build IS NOT NULL THEN NULL
    WHEN ($1).major <> 0 THEN ROW(($1).major, 0, 0, NULL, NULL)::semver
    WHEN ($1).minor <> 0 THEN ROW(0, ($1).minor, 0, NULL, NULL)::semver
    ELSE $1
  END $$ LANGUAGE SQL IMMUTABLE;
WITH non_betas AS (
  SELECT id,
    package_id,
    semver,
    created
  FROM versions
  WHERE (semver).prerelease IS NULL
    AND (semver).build IS NULL
    AND (
      package_id = 2451293
      OR package_id = 2717929
    )
) CREATE TEMP TABLE analysis.non_betas_with_ordering AS
SELECT analysis.base_compatible_semver(semver) AS group_base_semver,
  ROW_NUMBER() OVER(
    PARTITION BY package_id,
    analysis.base_compatible_semver(semver)
    ORDER BY created
  ) AS chron_order_within_group,
  ROW_NUMBER() OVER(
    PARTITION BY package_id
    ORDER BY created
  ) AS chron_order_global,
  ROW_NUMBER() OVER(
    PARTITION BY package_id,
    analysis.base_compatible_semver(semver)
    ORDER BY semver
  ) AS semver_order_within_group,
  ROW_NUMBER() OVER(
    PARTITION BY package_id
    ORDER BY semver
  ) AS semver_order_global,
  DENSE_RANK() OVER(
    PARTITION BY package_id
    ORDER BY analysis.base_compatible_semver(semver)
  ) AS inter_group_order,
  ROW_NUMBER() OVER(
    PARTITION BY package_id,
    analysis.base_compatible_semver(semver)
    ORDER BY semver DESC
  ) AS rev_semver_order_within_group,
  id,
  package_id,
  semver,
  created
FROM non_betas;
CREATE TEMP TABLE analysis.version_counts AS
SELECT package_id,
  COUNT(*) AS version_count,
  COUNT(DISTINCT group_base_semver) AS group_count
FROM non_betas_with_ordering
GROUP BY package_id;
CREATE TEMP TABLE analysis.group_ranges AS
SELECT group_start.group_base_semver AS group_base_semver,
  group_start.chron_order_global AS start_chron_order_global,
  group_end.chron_order_global AS end_chron_order_global,
  group_end.semver_order_global AS end_semver_order_global,
  group_start.inter_group_order AS inter_group_order,
  group_start.package_id AS package_id,
  group_start.id AS start_id,
  group_end.id AS end_id,
  group_start.created AS start_created,
  group_end.created AS end_created
FROM non_betas_with_ordering group_start
  INNER JOIN non_betas_with_ordering group_end ON group_start.package_id = group_end.package_id
  AND group_start.group_base_semver = group_end.group_base_semver
  AND group_start.semver_order_within_group = 1
  AND group_end.rev_semver_order_within_group = 1;
WITH intra_group_correct_version_order_counts AS (
  SELECT package_id,
    COUNT(*) AS correct_version_count
  FROM non_betas_with_ordering
  WHERE chron_order_within_group = semver_order_within_group
  GROUP BY package_id
),
valid_inter_group_order_counts AS (
  SELECT from_group.package_id,
    COUNT(*) AS group_trans_count
  FROM group_ranges from_group
    INNER JOIN group_ranges to_group ON from_group.package_id = to_group.package_id
    AND from_group.inter_group_order + 1 = to_group.inter_group_order
  WHERE from_group.start_created < to_group.start_created
  GROUP BY from_group.package_id
) CREATE TABLE analysis.valid_packages AS
SELECT version_counts.package_id
FROM version_counts
  INNER JOIN intra_group_correct_version_order_counts ON version_counts.package_id = intra_group_correct_version_order_counts.package_id
  LEFT JOIN valid_inter_group_order_counts ON version_counts.package_id = valid_inter_group_order_counts.package_id
WHERE correct_version_count = version_count
  AND coalesce(group_trans_count, 0) + 1 = group_count;
-- 2068382  
CREATE TABLE analysis.malformed_packages AS
SELECT package_id
FROM version_counts
WHERE package_id NOT IN (
    SELECT *
    FROM valid_packages
  );
CREATE TABLE analysis.valid_non_betas_with_ordering AS
SELECT group_base_semver,
  inter_group_order,
  semver_order_within_group AS order_within_group,
  chron_order_global,
  semver_order_global,
  id,
  package_id,
  semver,
  created
FROM non_betas_with_ordering
WHERE package_id IN (
    SELECT *
    FROM valid_packages
  );
CREATE TABLE analysis.valid_group_ranges AS
SELECT *
FROM group_ranges
WHERE package_id IN (
    SELECT *
    FROM valid_packages
  );