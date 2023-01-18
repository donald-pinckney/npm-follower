CREATE TABLE analysis.possible_install_deps AS WITH first_step_deps AS(
    SELECT pkg,
        depends_on_pkg
    FROM analysis.possible_direct_runtime_deps
    UNION
    SELECT pkg,
        depends_on_pkg
    FROM analysis.possible_direct_dev_deps
)
SELECT pkg,
    depends_on_pkg
FROM first_step_deps
UNION
SELECT fs.pkg,
    trans_relation.depends_on_pkg
FROM first_step_deps fs
    INNER JOIN analysis.possible_transitive_runtime_deps trans_relation ON fs.depends_on_pkg = trans_relation.pkg;

ALTER TABLE analysis.possible_install_deps
ADD PRIMARY KEY (pkg, depends_on_pkg);

ALTER TABLE analysis.possible_install_deps
ADD CONSTRAINT analysis_possible_install_deps_fkey_pkg FOREIGN KEY (pkg) REFERENCES packages (id);

ALTER TABLE analysis.possible_install_deps
ADD CONSTRAINT analysis_possible_install_deps_fkey_depends_on_pkg FOREIGN KEY (depends_on_pkg) REFERENCES packages (id);

ANALYZE analysis.possible_install_deps;

GRANT SELECT ON analysis.possible_install_deps TO data_analyzer;

