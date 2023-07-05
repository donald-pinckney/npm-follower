-- Bug: we don't actually check dependencies *at the time of unpublish*
-- So there are some rows in the result that actually are within policy.
CREATE TABLE metadata_analysis.out_of_policy_unpublish as with num_dependents as (
    select
        depends_on_pkg as pkg,
        count(*) as dependents
    from
        metadata_analysis.possible_direct_any_deps_non_deleted
    group by
        depends_on_pkg
)
select
    uv.*,
    nd.dependents
from
    metadata_analysis.unpublished_versions uv
    join num_dependents nd on nd.pkg = uv.package_id
where
    (uv.v).prerelease IS NULL
    and (uv.v).build IS NULL
    and delete_delay > '3 day'