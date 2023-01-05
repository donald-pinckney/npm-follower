CREATE UNLOGGED TABLE analysis.possible_transitive_deps AS WITH RECURSIVE search_graph(from_id, to_id) AS (
    SELECT g.from_id,
        g.to_id
    FROM analysis.possible_direct_deps g
    UNION
    SELECT g.from_id,
        g.to_id
    FROM analysis.possible_direct_deps g,
        search_graph sg
    WHERE g.from_id = sg.to_id
)
SELECT *
FROM search_graph;


ALTER TABLE analysis.possible_transitive_deps
ALTER COLUMN pkg
SET NOT NULL;

ALTER TABLE analysis.possible_transitive_deps
ALTER COLUMN depends_on_pkg
SET NOT NULL;


GRANT SELECT ON analysis.possible_transitive_deps TO data_analyzer;
GRANT ALL ON analysis.possible_transitive_deps TO pinckney;
GRANT ALL ON analysis.possible_transitive_deps TO federico;