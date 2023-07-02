CREATE TABLE metadata_analysis.possible_install_deps AS WITH first_step_deps AS(
    SELECT pkg,
        depends_on_pkg
    FROM metadata_analysis.possible_direct_runtime_deps
    UNION
    SELECT pkg,
        depends_on_pkg
    FROM metadata_analysis.possible_direct_dev_deps
)
SELECT pkg,
    depends_on_pkg
FROM first_step_deps
UNION
SELECT fs.pkg,
    trans_relation.depends_on_pkg
FROM first_step_deps fs
    INNER JOIN metadata_analysis.possible_transitive_runtime_deps trans_relation ON fs.depends_on_pkg = trans_relation.pkg;

ALTER TABLE metadata_analysis.possible_install_deps
ADD PRIMARY KEY (pkg, depends_on_pkg);

ALTER TABLE metadata_analysis.possible_install_deps
ADD CONSTRAINT metadata_analysis_possible_install_deps_fkey_pkg FOREIGN KEY (pkg) REFERENCES packages (id);

ALTER TABLE metadata_analysis.possible_install_deps
ADD CONSTRAINT metadata_analysis_possible_install_deps_fkey_depends_on_pkg FOREIGN KEY (depends_on_pkg) REFERENCES packages (id);

ANALYZE metadata_analysis.possible_install_deps;

GRANT SELECT ON metadata_analysis.possible_install_deps TO data_analyzer;
GRANT ALL ON metadata_analysis.possible_install_deps TO pinckney;
GRANT ALL ON metadata_analysis.possible_install_deps TO federico;

