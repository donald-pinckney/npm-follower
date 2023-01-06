CREATE TABLE analysis.possible_transitive_deps AS WITH RECURSIVE search_graph(pkg, depends_on_pkg) AS (
    SELECT g.pkg,
        g.depends_on_pkg
    FROM analysis.possible_direct_deps g
    UNION ALL
    SELECT sg.pkg,
        g.depends_on_pkg
    FROM search_graph sg
        INNER JOIN analysis.possible_direct_deps g ON sg.depends_on_pkg = g.pkg
) CYCLE pkg
SET is_cycle USING path
SELECT DISTINCT pkg,
    depends_on_pkg
FROM search_graph;


ALTER TABLE analysis.possible_transitive_deps
ADD PRIMARY KEY (pkg, depends_on_pkg);

ALTER TABLE analysis.possible_transitive_deps
ADD CONSTRAINT analysis_possible_transitive_deps_fkey_pkg FOREIGN KEY (pkg) REFERENCES packages (id);

ALTER TABLE analysis.possible_transitive_deps
ADD CONSTRAINT analysis_possible_transitive_deps_fkey_depends_on_pkg FOREIGN KEY (depends_on_pkg) REFERENCES packages (id);

ANALYZE analysis.possible_transitive_deps;

GRANT SELECT ON analysis.possible_transitive_deps TO data_analyzer;
GRANT ALL ON analysis.possible_transitive_deps TO pinckney;
GRANT ALL ON analysis.possible_transitive_deps TO federico;