-- this is a psql script.

-- function that given a constraint string (e.g. "^1.2.3"), gives the contraint type ("^")
-- there are 8 types of constraints: ^, ~, <, >, =, <=, >=, and *. we will check if 
-- the string contains either of them, and return the first one we find. if none are found,
-- we return NULL.
-- also, we have `A || B`, `A, B`, `A - B`, and others.
CREATE OR REPLACE FUNCTION analysis.check_constraint_type(constraint_string text) RETURNS text AS $$
    SELECT CASE 
    WHEN constraint_string LIKE '%|%' THEN
        '||'
    WHEN constraint_string LIKE '%,%' THEN
        ','
    WHEN constraint_string LIKE '% - %' THEN
        '-'
    WHEN constraint_string LIKE '"^%' OR constraint_string LIKE '" ^%' THEN
        'minor'
    WHEN constraint_string LIKE '"~%' OR constraint_string LIKE '" ~%' THEN
        'patch'
    WHEN constraint_string LIKE '"<=%' OR constraint_string LIKE '" <=%' THEN
        '<='
    WHEN constraint_string LIKE '">=%' OR constraint_string LIKE '" >=%' THEN
        '>='
    WHEN constraint_string LIKE '"<%' OR constraint_string LIKE '" <%' THEN
        '<'
    WHEN constraint_string LIKE '">%' OR constraint_string LIKE '" >%' THEN
        '>'
    WHEN constraint_string LIKE '"=%' OR constraint_string LIKE '" =%' THEN
        '='
    WHEN constraint_string ~ '^"\ *v?[0-9]+\.[0-9]+\.[x||X||\*]"$' THEN
        'patch'
    WHEN constraint_string ~ '^"\ *v?[0-9]+\.[x||X||\*](\.[x||X||\*])?"$' THEN
        'minor'
    WHEN constraint_string ~ '^"\ *v?[x||X||\*]\.[x||X||\*]\.[x||X||\*]"$' THEN
        'major' -- same thing as '*'
    WHEN constraint_string ~ '^"\ *v?[0-9]+\.[0-9]+"$' THEN
        'patch'
    WHEN constraint_string ~ '^"\ *v?[0-9]+"$' THEN
        'minor'
    WHEN constraint_string ~ '^"\ *v?[0-9]+\.[0-9]+\.[0-9]+-?.*"$' THEN
        '=' -- essentially a `=`
    WHEN constraint_string LIKE '"*"' 
      OR constraint_string LIKE '" *"' 
      OR constraint_string LIKE '""' 
      OR constraint_string LIKE '"x"' 
      OR constraint_string LIKE '" x"'
      OR constraint_string LIKE '"x.*"'
      OR constraint_string LIKE '" x.*"'
      OR constraint_string LIKE '"x.x"'
      OR constraint_string LIKE '" x.x"' THEN 
        'major'
    ELSE
        'invalid'
END
$$ LANGUAGE SQL IMMUTABLE;


CREATE TABLE analysis.constraint_types AS
SELECT 
    id as dependency_id, 
    CASE WHEN (spec).dep_type = 'range' AND 
              jsonb_typeof(raw_spec) = 'string' 
              THEN analysis.check_constraint_type(raw_spec #>> '{}')
         ELSE NULL
    END as constraint_type
FROM dependencies;



CREATE INDEX constraint_types_idx ON analysis.constraint_types (dependency_id);

ANALYZE analysis.constraint_types;

GRANT SELECT ON analysis.constraint_types TO data_analyzer;
GRANT ALL ON analysis.constraint_types TO pinckney;
GRANT ALL ON analysis.constraint_types TO federico;


    