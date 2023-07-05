
-- We need to find seqs s such that:
-- diff_log where seq == s looks like:
-- id	+ 0, s, name, delete_version, NULL,    v0, NULL
-- id	+ 1, s, name, delete_version, NULL,    v1, NULL
-- ...
-- id	+ n, s, name, delete_version, NULL,    vn, NULL
-- id+n+1, s, name, update_package, pack, NULL, NULL

-- NO delete_package diff entry

-- Example:

-- | id       | seq      | package_name      | dt             | package_only_packument | v         | version_packument |
-- | -------- | -------- | ----------------- | -------------- | ---------------------- | --------- | ----------------- |
-- | 58774552 | 24730339 | testing-unpublish | delete_version |                        | (1,0,1,,) |                   |
-- | 58774553 | 24730339 | testing-unpublish | update_package |          ...           |           |                   |

CREATE TABLE metadata_analysis.unpublished_versions AS
with version_unpublish_seqs as (
  select 
    seq, 
    sum(case when dt = 'delete_version' then 1 else 0 end) as num_delete,
    min(case when dt = 'update_package' then package_only_packument #>> '{Normal,modified}' else null end)::timestamp with time zone as modified_time
  from diff_log 
  group by seq
  having 
    coalesce(bool_or(dt = 'delete_version'), false) and 
    coalesce(bool_or(dt = 'update_package' and package_only_packument <> '"Deleted"'), false) and 
    coalesce(bool_and(dt = 'delete_version' or (dt = 'update_package' and package_only_packument <> '"Deleted"')), false)
)
select 
    de.package_name, 
    p.id as package_id, 
    de.v, 
    ver.id as version_id, 
    us.modified_time - ver.created as delete_delay
from version_unpublish_seqs us 
join diff_log de
on us.seq = de.seq and de.dt = 'delete_version'
join packages p
on de.package_name = p.name
join versions ver
on ver.package_id = p.id and de.v = ver.semver;


ALTER TABLE metadata_analysis.unpublished_versions
ADD PRIMARY KEY (version_id);

ANALYZE metadata_analysis.unpublished_versions;

GRANT SELECT ON metadata_analysis.unpublished_versions TO data_analyzer;
GRANT ALL ON metadata_analysis.unpublished_versions TO pinckney;
GRANT ALL ON metadata_analysis.unpublished_versions TO federico;