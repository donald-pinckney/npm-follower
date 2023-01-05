CREATE UNLOGGED TABLE analysis.possible_direct_deps AS WITH deps_of_package AS (
    SELECT package_id,
        unnest(
            prod_dependencies || dev_dependencies || optional_dependencies || peer_dependencies
        ) AS dep_id
    FROM versions
)
SELECT DISTINCT src_dep.package_id pkg,
    d.dst_package_id_if_exists AS depends_on_pkg
FROM deps_of_package src_dep
    INNER JOIN dependencies d ON src_dep.dep_id = d.id
WHERE d.dst_package_id_if_exists IS NOT NULL;