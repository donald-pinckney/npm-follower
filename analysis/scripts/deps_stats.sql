CREATE TABLE analysis.all_dep_counts AS with direct_runtime_dep_counts_exist as (
  select pkg,
    count(*) as x,
    'num_direct_runtime_deps' as count_type
  from analysis.possible_direct_runtime_deps
  group by pkg
),
direct_runtime_rev_dep_counts_exist as (
  select depends_on_pkg as pkg,
    count(*) as x,
    'num_direct_runtime_rev_deps' as count_type
  from analysis.possible_direct_runtime_deps
  group by depends_on_pkg
),
direct_dev_dep_counts_exist as (
  select pkg,
    count(*) as x,
    'num_direct_dev_deps' as count_type
  from analysis.possible_direct_dev_deps
  group by pkg
),
direct_dev_rev_dep_counts_exist as (
  select depends_on_pkg as pkg,
    count(*) as x,
    'num_direct_dev_rev_deps' as count_type
  from analysis.possible_direct_dev_deps
  group by depends_on_pkg
),
install_dep_counts_exist as (
  select pkg,
    count(*) as x,
    'num_install_deps' as count_type
  from analysis.possible_install_deps
  group by pkg
),
install_rev_dep_counts_exist as (
  select depends_on_pkg as pkg,
    count(*) as x,
    'num_install_rev_deps' as count_type
  from analysis.possible_install_deps
  group by depends_on_pkg
),
transitive_runtime_dep_counts_exist as (
  select pkg,
    count(*) as x,
    'num_transitive_runtime_deps' as count_type
  from analysis.possible_transitive_runtime_deps
  group by pkg
),
transitive_runtime_rev_dep_counts_exist as (
  select depends_on_pkg as pkg,
    count(*) as x,
    'num_transitive_runtime_rev_deps' as count_type
  from analysis.possible_transitive_runtime_deps
  group by depends_on_pkg
),
direct_runtime_dep_counts as (
  select *
  from direct_runtime_dep_counts_exist
  union all
  select id,
    0 as x,
    'num_direct_runtime_deps' as count_type
  from packages
  where id not in (
      select pkg
      from direct_runtime_dep_counts_exist
    )
),
direct_runtime_rev_dep_counts as (
  select *
  from direct_runtime_rev_dep_counts_exist
  union all
  select id,
    0 as x,
    'num_direct_runtime_rev_deps' as count_type
  from packages
  where id not in (
      select pkg
      from direct_runtime_rev_dep_counts_exist
    )
),
direct_dev_dep_counts as (
  select *
  from direct_dev_dep_counts_exist
  union all
  select id,
    0 as x,
    'num_direct_dev_deps' as count_type
  from packages
  where id not in (
      select pkg
      from direct_dev_dep_counts_exist
    )
),
direct_dev_rev_dep_counts as (
  select *
  from direct_dev_rev_dep_counts_exist
  union all
  select id,
    0 as x,
    'num_direct_dev_rev_deps' as count_type
  from packages
  where id not in (
      select pkg
      from direct_dev_rev_dep_counts_exist
    )
),
install_dep_counts as (
  select *
  from install_dep_counts_exist
  union all
  select id,
    0 as x,
    'num_install_deps' as count_type
  from packages
  where id not in (
      select pkg
      from install_dep_counts_exist
    )
),
install_rev_dep_counts as (
  select *
  from install_rev_dep_counts_exist
  union all
  select id,
    0 as x,
    'num_install_rev_deps' as count_type
  from packages
  where id not in (
      select pkg
      from install_rev_dep_counts_exist
    )
),
transitive_runtime_dep_counts as (
  select *
  from transitive_runtime_dep_counts_exist
  union all
  select id,
    0 as x,
    'num_transitive_runtime_deps' as count_type
  from packages
  where id not in (
      select pkg
      from transitive_runtime_dep_counts_exist
    )
),
transitive_runtime_rev_dep_counts as (
  select *
  from transitive_runtime_rev_dep_counts_exist
  union all
  select id,
    0 as x,
    'num_transitive_runtime_rev_deps' as count_type
  from packages
  where id not in (
      select pkg
      from transitive_runtime_rev_dep_counts_exist
    )
) (
  select *
  from direct_runtime_dep_counts
  union all
  select *
  from direct_runtime_rev_dep_counts
  union all
  select *
  from direct_dev_dep_counts
  union all
  select *
  from direct_dev_rev_dep_counts
  union all
  select *
  from install_dep_counts
  union all
  select *
  from install_rev_dep_counts
  union all
  select *
  from transitive_runtime_dep_counts
  union all
  select *
  from transitive_runtime_rev_dep_counts
);


GRANT SELECT ON analysis.all_dep_counts TO data_analyzer;
GRANT ALL ON analysis.all_dep_counts TO pinckney;
GRANT ALL ON analysis.all_dep_counts TO federico;


ALTER TABLE analysis.all_dep_counts
ADD PRIMARY KEY (pkg, count_type);

ANALYZE analysis.all_dep_counts;



CREATE TABLE analysis.deps_stats AS WITH computed_stats_wide as (
  select count_type,
    avg(x),
    min(x),
    max(x),
    stddev_pop(x),
    mode() within group(
      order by x
    ),
    percentile_cont(ARRAY [0.05, 0.25, 0.5, 0.75, 0.95]) within group(
      order by x
    ) as percentiles_5_25_50_75_95
  from analysis.all_dep_counts
  group by count_type
),
computed_stats as (
  select count_type,
    'avg' as statistic_name,
    avg as value
  from computed_stats_wide
  union all
  select count_type,
    'min' as statistic_name,
    min as value
  from computed_stats_wide
  union all
  select count_type,
    'max' as statistic_name,
    max as value
  from computed_stats_wide
  union all
  select count_type,
    'stddev_pop' as statistic_name,
    stddev_pop as value
  from computed_stats_wide
  union all
  select count_type,
    'mode' as statistic_name,
    mode as value
  from computed_stats_wide
  union all
  select count_type,
    '5th_percentile' as statistic_name,
    percentiles_5_25_50_75_95 [1] as value
  from computed_stats_wide
  union all
  select count_type,
    '25th_percentile' as statistic_name,
    percentiles_5_25_50_75_95 [2] as value
  from computed_stats_wide
  union all
  select count_type,
    '50th_percentile' as statistic_name,
    percentiles_5_25_50_75_95 [3] as value
  from computed_stats_wide
  union all
  select count_type,
    '75th_percentile' as statistic_name,
    percentiles_5_25_50_75_95 [4] as value
  from computed_stats_wide
  union all
  select count_type,
    '95th_percentile' as statistic_name,
    percentiles_5_25_50_75_95 [5] as value
  from computed_stats_wide
),
nearest_example_values as (
  select distinct on (s.count_type, s.statistic_name) s.count_type,
    s.statistic_name,
    s.value,
    c.x as nearest_real_value
  from computed_stats s
    inner join analysis.all_dep_counts c on s.count_type = c.count_type
    and s.statistic_name <> 'stddev_pop'
  order by s.count_type,
    s.statistic_name,
    abs(s.value - c.x)
)
select distinct on (s.count_type, s.statistic_name) s.count_type,
  s.statistic_name,
  s.value,
  e.nearest_real_value as example_value,
  c.pkg as example_pkg_id,
  p.name as example_pkg
from computed_stats s
  left join nearest_example_values e on s.count_type = e.count_type
  and s.statistic_name = e.statistic_name
  left join analysis.all_dep_counts c on c.count_type = s.count_type
  and e.nearest_real_value = c.x
  left join packages p on c.pkg = p.id
order by s.count_type,
  s.statistic_name;


GRANT SELECT ON analysis.deps_stats TO data_analyzer;
GRANT ALL ON analysis.deps_stats TO pinckney;
GRANT ALL ON analysis.deps_stats TO federico;