CREATE TABLE solving_analysis.subsampled_updates AS WITH filtered_updates as (
  SELECT *
  from metadata_analysis.all_updates
  where to_created < TIMESTAMP WITH TIME ZONE '2021-01-01 00:00:00+00'
    and ty <> 'zero_to_something'
    and ROW(from_id, to_id) NOT IN (
      SELECT from_id,
        to_id
      FROM metadata_analysis.vuln_patch_updates
    )
    and ROW(from_id, to_id) NOT IN (
      SELECT from_id,
        to_id
      FROM metadata_analysis.vuln_intro_updates
    )
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
  ty,
  FALSE as patches_vuln,
  FALSE as introduced_vuln
FROM ranked_updates
WHERE date_rank = 1
UNION ALL
SELECT DISTINCT package_id,
  from_id,
  to_id,
  from_semver,
  to_semver,
  from_created,
  to_created,
  ty,
  TRUE as patches_vuln,
  FALSE as introduced_vuln
FROM metadata_analysis.vuln_patch_updates
where ROW(from_id, to_id) NOT IN (
    SELECT from_id,
      to_id
    FROM metadata_analysis.vuln_intro_updates
  )
UNION ALL
SELECT DISTINCT package_id,
  from_id,
  to_id,
  from_semver,
  to_semver,
  from_created,
  to_created,
  ty,
  FALSE as patches_vuln,
  TRUE as introduced_vuln
FROM metadata_analysis.vuln_intro_updates
where ROW(from_id, to_id) NOT IN (
    SELECT from_id,
      to_id
    FROM metadata_analysis.vuln_patch_updates
  )
UNION ALL
SELECT DISTINCT i.package_id,
  i.from_id,
  i.to_id,
  i.from_semver,
  i.to_semver,
  i.from_created,
  i.to_created,
  i.ty,
  TRUE as patches_vuln,
  TRUE as introduced_vuln
FROM metadata_analysis.vuln_intro_updates i
  inner join metadata_analysis.vuln_patch_updates p on i.from_id = p.from_id
  and i.to_id = p.to_id;

ALTER TABLE solving_analysis.subsampled_updates
ADD PRIMARY KEY (from_id, to_id);

ANALYZE solving_analysis.subsampled_updates;

GRANT SELECT ON solving_analysis.subsampled_updates TO data_analyzer;
GRANT ALL ON solving_analysis.subsampled_updates TO pinckney;
GRANT ALL ON solving_analysis.subsampled_updates TO federico;