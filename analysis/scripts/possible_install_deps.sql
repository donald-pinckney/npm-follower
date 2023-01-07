CREATE TABLE analysis.possible_install_deps AS WITH first_step_deps AS(
    SELECT version_id,
        depends_on_pkg
    FROM analysis.possible_direct_runtime_version_deps
    UNION
    SELECT version_id,
        depends_on_pkg
    FROM analysis.possible_direct_dev_version_deps
)
SELECT version_id,
    depends_on_pkg
FROM first_step_deps
UNION
SELECT fs.version_id,
    trans_relation.depends_on_pkg
FROM first_step_deps fs
    INNER JOIN analysis.possible_transitive_runtime_deps trans_relation ON fs.depends_on_pkg = trans_relation.pkg;

ALTER TABLE analysis.possible_install_deps
ADD PRIMARY KEY (version_id, depends_on_pkg);

ALTER TABLE analysis.possible_install_deps
ADD CONSTRAINT analysis_possible_install_deps_fkey_version_id FOREIGN KEY (version_id) REFERENCES versions (id);

ALTER TABLE analysis.possible_install_deps
ADD CONSTRAINT analysis_possible_install_deps_fkey_depends_on_pkg FOREIGN KEY (depends_on_pkg) REFERENCES packages (id);

ANALYZE analysis.possible_install_deps;

GRANT SELECT ON analysis.possible_install_deps TO data_analyzer;
GRANT ALL ON analysis.possible_install_deps TO pinckney;
GRANT ALL ON analysis.possible_install_deps TO federico;