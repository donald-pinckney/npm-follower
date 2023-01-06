CREATE TABLE analysis.possible_direct_dev_version_deps AS WITH deps_of_version AS (
    SELECT id as version_id,
        unnest(dev_dependencies) AS dep_id
    FROM versions
)
SELECT DISTINCT src_dep.version_id AS version_id,
    d.dst_package_id_if_exists AS depends_on_pkg
FROM deps_of_version src_dep
    INNER JOIN dependencies d ON src_dep.dep_id = d.id
WHERE d.dst_package_id_if_exists IS NOT NULL;

ALTER TABLE analysis.possible_direct_dev_version_deps
ADD PRIMARY KEY (version_id, depends_on_pkg);

ALTER TABLE analysis.possible_direct_dev_version_deps
ADD CONSTRAINT analysis_possible_direct_dev_version_deps_fkey_version_id FOREIGN KEY (version_id) REFERENCES versions (id);
ALTER TABLE analysis.possible_direct_dev_version_deps
ADD CONSTRAINT analysis_possible_direct_dev_version_deps_fkey_depends_on_pkg FOREIGN KEY (depends_on_pkg) REFERENCES packages (id);


ANALYZE analysis.possible_direct_dev_version_deps;


GRANT SELECT ON analysis.possible_direct_dev_version_deps TO data_analyzer;
GRANT ALL ON analysis.possible_direct_dev_version_deps TO pinckney;
GRANT ALL ON analysis.possible_direct_dev_version_deps TO federico;