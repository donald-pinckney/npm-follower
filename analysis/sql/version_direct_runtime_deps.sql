CREATE TABLE metadata_analysis.version_direct_runtime_deps AS WITH deps_of_version AS (
    SELECT id as version_id,
        unnest(
            prod_dependencies || optional_dependencies || peer_dependencies
        ) AS dep_id
    FROM versions
)
SELECT DISTINCT src_dep.version_id AS v,
    d.dst_package_id_if_exists AS depends_on_pkg
FROM deps_of_version src_dep
    INNER JOIN dependencies d ON src_dep.dep_id = d.id
WHERE d.dst_package_id_if_exists IS NOT NULL;

ALTER TABLE metadata_analysis.version_direct_runtime_deps
ADD PRIMARY KEY (v, depends_on_pkg);

ALTER TABLE metadata_analysis.version_direct_runtime_deps
ADD CONSTRAINT metadata_analysis_version_direct_runtime_deps_fkey_pkg FOREIGN KEY (v) REFERENCES versions (id);
ALTER TABLE metadata_analysis.version_direct_runtime_deps
ADD CONSTRAINT metadata_analysis_version_direct_runtime_deps_fkey_depends_on_pkg FOREIGN KEY (depends_on_pkg) REFERENCES packages (id);


ANALYZE metadata_analysis.version_direct_runtime_deps;


GRANT SELECT ON metadata_analysis.version_direct_runtime_deps TO data_analyzer;
GRANT ALL ON metadata_analysis.version_direct_runtime_deps TO pinckney;
GRANT ALL ON metadata_analysis.version_direct_runtime_deps TO federico;