CREATE TABLE analysis.full_dependency_diffs AS (

with deps_long as (
  select v.id as v_id, 'prod' as dep_type, unnest(v.prod_dependencies) as dep_id from versions v
  union all
  select v.id as v_id, 'dev' as dep_type, unnest(v.dev_dependencies) as dep_id from versions v
  union all
  select v.id as v_id, 'optional' as dep_type, unnest(v.optional_dependencies) as dep_id from versions v
  union all
  select v.id as v_id, 'peer' as dep_type, unnest(v.peer_dependencies) as dep_id from versions v
),

deps_long_full_info as (
  select 
  distinct on (vd.v_id, vd.dep_type, d.dst_package_id_if_exists) 
  vd.v_id as v_id, vd.dep_type as dep_type, d.dst_package_id_if_exists as dep_on_pkg, d.raw_spec as raw_spec 
  from deps_long vd
  inner join dependencies d
  on d.id = vd.dep_id
  where d.dst_package_id_if_exists is not null
  -- limit(1000)
)

select 
	u.from_id as from_id, 
    u.to_id as to_id, 
    from_v.dep_type as dep_type, 
    from_v.dep_on_pkg as dep_on_pkg, 
    from_v.raw_spec as from_spec, 
    to_v.raw_spec as to_spec
from analysis.all_updates u
inner join deps_long_full_info from_v on u.from_id = from_v.v_id
inner join deps_long_full_info to_v on u.to_id = to_v.v_id and to_v.dep_type = from_v.dep_type and to_v.dep_on_pkg = from_v.dep_on_pkg
where from_v.raw_spec <> to_v.raw_spec

union all

select
	u.from_id as from_id, 
    u.to_id as to_id, 
    from_v.dep_type as dep_type, 
    from_v.dep_on_pkg as dep_on_pkg, 
    from_v.raw_spec as from_spec, 
    NULL as to_spec
from analysis.all_updates u
inner join deps_long_full_info from_v on u.from_id = from_v.v_id
where ROW(u.to_id, from_v.dep_type, from_v.dep_on_pkg) NOT IN (select v_id, dep_type, dep_on_pkg from deps_long_full_info)

union all

select
	u.from_id as from_id, 
    u.to_id as to_id, 
    to_v.dep_type as dep_type, 
    to_v.dep_on_pkg as dep_on_pkg, 
    NULL as from_spec, 
    to_v.raw_spec as to_spec
from analysis.all_updates u
inner join deps_long_full_info to_v on u.to_id = to_v.v_id
where ROW(u.from_id, to_v.dep_type, to_v.dep_on_pkg) NOT IN (select v_id, dep_type, dep_on_pkg from deps_long_full_info)

);