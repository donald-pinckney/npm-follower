CREATE UNLOGGED TABLE analysis.possible_direct_deps AS WITH deps_of_package AS (
    SELECT package_id,
        unnest(
            prod_dependencies || dev_dependencies || optional_dependencies || peer_dependencies
        ) AS dep_id
    FROM versions
)
SELECT DISTINCT src_dep.package_id AS pkg,
    d.dst_package_id_if_exists AS depends_on_pkg
FROM deps_of_package src_dep
    INNER JOIN dependencies d ON src_dep.dep_id = d.id
WHERE d.dst_package_id_if_exists IS NOT NULL;

ALTER TABLE analysis.possible_direct_deps
ALTER COLUMN pkg
SET NOT NULL;

ALTER TABLE analysis.possible_direct_deps
ALTER COLUMN depends_on_pkg
SET NOT NULL;

CREATE INDEX analysis_possible_direct_deps_idx_pkg ON analysis.possible_direct_deps (pkg);
CREATE INDEX analysis_possible_direct_deps_idx_depends_on_pkg ON analysis.possible_direct_deps (depends_on_pkg);

ANALYZE analysis.possible_direct_deps;


GRANT SELECT ON analysis.possible_direct_deps TO data_analyzer;
GRANT ALL ON analysis.possible_direct_deps TO pinckney;
GRANT ALL ON analysis.possible_direct_deps TO federico;