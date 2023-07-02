CREATE TABLE metadata_analysis.possible_transitive_runtime_deps AS WITH RECURSIVE search_graph(pkg, depends_on_pkg) AS (
    SELECT g.pkg,
        g.depends_on_pkg
    FROM metadata_analysis.possible_direct_runtime_deps g
    UNION ALL
    SELECT sg.pkg,
        g.depends_on_pkg
    FROM search_graph sg
        INNER JOIN metadata_analysis.possible_direct_runtime_deps g ON sg.depends_on_pkg = g.pkg
) CYCLE pkg
SET is_cycle USING path
SELECT DISTINCT pkg,
    depends_on_pkg
FROM search_graph;


ALTER TABLE metadata_analysis.possible_transitive_runtime_deps
ADD PRIMARY KEY (pkg, depends_on_pkg);

ALTER TABLE metadata_analysis.possible_transitive_runtime_deps
ADD CONSTRAINT metadata_analysis_possible_transitive_runtime_deps_fkey_pkg FOREIGN KEY (pkg) REFERENCES packages (id);

ALTER TABLE metadata_analysis.possible_transitive_runtime_deps
ADD CONSTRAINT metadata_analysis_possible_transitive_runtime_deps_fkey_depends_on_pkg FOREIGN KEY (depends_on_pkg) REFERENCES packages (id);

ANALYZE metadata_analysis.possible_transitive_runtime_deps;

GRANT SELECT ON metadata_analysis.possible_transitive_runtime_deps TO data_analyzer;
GRANT ALL ON metadata_analysis.possible_transitive_runtime_deps TO pinckney;
GRANT ALL ON metadata_analysis.possible_transitive_runtime_deps TO federico;