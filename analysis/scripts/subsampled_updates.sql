CREATE TABLE analysis.subsampled_updates WITH filtered_updates as (
  SELECT *
  from analysis.all_updates
  where to_created < TIMESTAMP WITH TIME ZONE '2021-01-01 00:00:00+00'
    and ty <> 'zero_to_something'
),
ranked_updates as (
  SELECT *,
    ROW_NUMBER() over (
      partition by package_id,
      ty
      order by to_created desc
    ) as date_rank
  FROM filtered_updates
)
SELECT package_id,
  from_id,
  to_id,
  from_semver,
  to_semver,
  from_created,
  to_created,
  ty
FROM ranked_updates
WHERE date_rank = 1;


ALTER TABLE analysis.subsampled_updates
ADD PRIMARY KEY (pkg, depends_on_pkg);

ALTER TABLE analysis.subsampled_updates
ADD CONSTRAINT analysis_subsampled_updates_fkey_pkg FOREIGN KEY (pkg) REFERENCES packages (id);

ALTER TABLE analysis.subsampled_updates
ADD CONSTRAINT analysis_subsampled_updates_fkey_depends_on_pkg FOREIGN KEY (depends_on_pkg) REFERENCES packages (id);

ANALYZE analysis.subsampled_updates;

GRANT SELECT ON analysis.subsampled_updates TO data_analyzer;
GRANT ALL ON analysis.subsampled_updates TO pinckney;
GRANT ALL ON analysis.subsampled_updates TO federico;