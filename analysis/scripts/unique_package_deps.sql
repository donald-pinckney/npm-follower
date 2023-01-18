CREATE TABLE analysis.unique_deps_across_versions AS

with 
distinct_deps as
(
    select distinct
    package_id,
    'prod' as dep_type,
    unnest(prod_dependencies) as dep 
    from versions

    union all

    select distinct
    package_id,
    'dev' as dep_type,
    unnest(dev_dependencies) as dep 
    from versions
    
    union all
    
    select distinct
    package_id,
    'optional' as dep_type,
    unnest(optional_dependencies) as dep 
    from versions
    
    union all
    
    select distinct
    package_id,
    'peer' as dep_type,
    unnest(peer_dependencies) as dep 
    from versions
)


select package_id, dependency_id, pd.dep_type, (spec).dep_type || coalesce('-' || constraint_type, '') as composite_constraint_type
from distinct_deps pd
inner join analysis.constraint_types ct
on pd.dep = ct.dependency_id
inner join dependencies d
on d.id = pd.dep;


GRANT SELECT ON analysis.unique_deps_across_versions TO data_analyzer;






CREATE TABLE analysis.unique_deps_of_latest AS

with versions_to_use as (
    select v.* 
    from packages p
    inner join versions v 
    on p.dist_tag_latest_version = v.id
),

distinct_deps as
(
    select distinct
    package_id,
    'prod' as dep_type,
    unnest(prod_dependencies) as dep 
    from versions_to_use

    union all

    select distinct
    package_id,
    'dev' as dep_type,
    unnest(dev_dependencies) as dep 
    from versions_to_use
    
    union all
    
    select distinct
    package_id,
    'optional' as dep_type,
    unnest(optional_dependencies) as dep 
    from versions_to_use
    
    union all
    
    select distinct
    package_id,
    'peer' as dep_type,
    unnest(peer_dependencies) as dep 
    from versions_to_use
)


select package_id, dependency_id, pd.dep_type, (spec).dep_type || coalesce('-' || constraint_type, '') as composite_constraint_type
from distinct_deps pd
inner join analysis.constraint_types ct
on pd.dep = ct.dependency_id
inner join dependencies d
on d.id = pd.dep;


GRANT SELECT ON analysis.unique_deps_of_latest TO data_analyzer;




CREATE TABLE analysis.unique_deps_yearly_latest AS

with ranked_vers as (
    select 
        ROW_NUMBER() OVER(PARTITION BY package_id, date_part('year', created) ORDER BY created desc) as r,
        date_part('year', created) as year,
        package_id,
        created,
        id as version_id
    from versions where (semver).prerelease IS NULL
    AND (semver).build IS NULL
),

versions_to_use as (
    select v.*, rv.year as year
    from versions v 
    inner join ranked_vers rv 
    on rv.version_id = v.id and rv.r = 1
),

distinct_deps as
(
    select distinct
    package_id,
    'prod' as dep_type,
    year,
    unnest(prod_dependencies) as dep
    from versions_to_use

    union all

    select distinct
    package_id,
    'dev' as dep_type,
    year,
    unnest(dev_dependencies) as dep 
    from versions_to_use
    
    union all
    
    select distinct
    package_id,
    'optional' as dep_type,
    year,
    unnest(optional_dependencies) as dep 
    from versions_to_use
    
    union all
    
    select distinct
    package_id,
    'peer' as dep_type,
    year,
    unnest(peer_dependencies) as dep 
    from versions_to_use
)


select package_id, pd.year, dependency_id, pd.dep_type, (spec).dep_type || coalesce('-' || constraint_type, '') as composite_constraint_type
from distinct_deps pd
inner join analysis.constraint_types ct
on pd.dep = ct.dependency_id
inner join dependencies d
on d.id = pd.dep;


GRANT SELECT ON analysis.unique_deps_yearly_latest TO data_analyzer;






CREATE TABLE analysis.unique_deps_yearly_latest_depended_on_only AS

with depdended_on_pkgs as (
    select distinct depends_on_pkg from analysis.possible_install_deps
),

ranked_vers as (
    select 
        ROW_NUMBER() OVER(PARTITION BY package_id, date_part('year', created) ORDER BY created desc) as r,
        date_part('year', created) as year,
        package_id,
        created,
        id as version_id
    from versions where package_id in (select * from depdended_on_pkgs)
),

versions_to_use as (
    select v.*, rv.year as year
    from versions v 
    inner join ranked_vers rv 
    on rv.version_id = v.id and rv.r = 1
),

distinct_deps as
(
    select distinct
    package_id,
    'prod' as dep_type,
    year,
    unnest(prod_dependencies) as dep
    from versions_to_use

    union all

    select distinct
    package_id,
    'dev' as dep_type,
    year,
    unnest(dev_dependencies) as dep 
    from versions_to_use
    
    union all
    
    select distinct
    package_id,
    'optional' as dep_type,
    year,
    unnest(optional_dependencies) as dep 
    from versions_to_use
    
    union all
    
    select distinct
    package_id,
    'peer' as dep_type,
    year,
    unnest(peer_dependencies) as dep 
    from versions_to_use
)


select package_id, pd.year, dependency_id, pd.dep_type, (spec).dep_type || coalesce('-' || constraint_type, '') as composite_constraint_type
from distinct_deps pd
inner join analysis.constraint_types ct
on pd.dep = ct.dependency_id
inner join dependencies d
on d.id = pd.dep;


GRANT SELECT ON analysis.unique_deps_yearly_latest_depended_on_only TO data_analyzer;







CREATE TABLE analysis.unique_deps_yearly_dep_on_vuln_pkg_only AS

with pkgs_with_vulns as (
    select distinct p.id 
    from ghsa a
    inner join vulnerabilities vuln on vuln.ghsa_id = a.id AND a.withdrawn_at IS NULL
    inner join packages p on p.name = vuln.package_name
),

ranked_vers as (
    select 
        ROW_NUMBER() OVER(PARTITION BY package_id, date_part('year', created) ORDER BY created desc) as r,
        date_part('year', created) as year,
        package_id,
        created,
        id as version_id
    from versions
),

versions_to_use as (
    select v.*, rv.year as year
    from versions v 
    inner join ranked_vers rv 
    on rv.version_id = v.id and rv.r = 1
),

distinct_deps as
(
    select distinct
    package_id,
    'prod' as dep_type,
    year,
    unnest(prod_dependencies) as dep
    from versions_to_use

    union all

    select distinct
    package_id,
    'dev' as dep_type,
    year,
    unnest(dev_dependencies) as dep 
    from versions_to_use
    
    union all
    
    select distinct
    package_id,
    'optional' as dep_type,
    year,
    unnest(optional_dependencies) as dep 
    from versions_to_use
    
    union all
    
    select distinct
    package_id,
    'peer' as dep_type,
    year,
    unnest(peer_dependencies) as dep 
    from versions_to_use
)


select package_id, pd.year, dependency_id, pd.dep_type, (spec).dep_type || coalesce('-' || constraint_type, '') as composite_constraint_type
from distinct_deps pd
inner join analysis.constraint_types ct
on pd.dep = ct.dependency_id
inner join dependencies d
on d.id = pd.dep and d.dst_package_id_if_exists IN (select * from pkgs_with_vulns);


GRANT SELECT ON analysis.unique_deps_yearly_dep_on_vuln_pkg_only TO data_analyzer;
