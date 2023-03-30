-- this is a psql script.

-- function that given a constraint string (e.g. "^1.2.3"), gives the contraint type ("^")
-- there are 8 types of constraints: ^, ~, <, >, =, <=, >=, and *. we will check if 
-- the string contains either of them, and return the first one we find. if none are found,
-- we return NULL.
-- also, we have `A || B`, `A, B`, `A - B`, and others.
CREATE OR REPLACE FUNCTION metadata_analysis.check_constraint_type(constraint_string text) RETURNS text AS $$
DECLARE
    constraint_type text;
BEGIN
    IF constraint_string LIKE '%|%' THEN
        constraint_type := '||';
    ELSIF constraint_string LIKE '%,%' THEN
        constraint_type := ',';
    ELSIF constraint_string LIKE '% - %' THEN
        constraint_type := '-';
    ELSIF constraint_string LIKE '"^%' OR constraint_string LIKE '" ^%' THEN
        constraint_type := 'minor';
    ELSIF constraint_string LIKE '"~%' OR constraint_string LIKE '" ~%' THEN
        constraint_type := 'patch';
    ELSIF constraint_string LIKE '"<=%' OR constraint_string LIKE '" <=%' THEN
        constraint_type := '<=';
    ELSIF constraint_string LIKE '">=%' OR constraint_string LIKE '" >=%' THEN
        constraint_type := '>=';
    ELSIF constraint_string LIKE '"<%' OR constraint_string LIKE '" <%' THEN
        constraint_type := '<';
    ELSIF constraint_string LIKE '">%' OR constraint_string LIKE '" >%' THEN
        constraint_type := '>';
    ELSIF constraint_string LIKE '"=%' OR constraint_string LIKE '" =%' THEN
        constraint_type := '=';
    ELSIF constraint_string ~ '^"\ *v?[0-9]+\.[0-9]+\.[x||X||\*]"$' THEN
        constraint_type := 'patch';
    ELSIF constraint_string ~ '^"\ *v?[0-9]+\.[x||X||\*](\.[x||X||\*])?"$' THEN
        constraint_type := 'minor';
    ELSIF constraint_string ~ '^"\ *v?[x||X||\*]\.[x||X||\*]\.[x||X||\*]"$' THEN
        constraint_type := 'major'; -- same thing as '*'
    ELSIF constraint_string ~ '^"\ *v?[0-9]+\.[0-9]+"$' THEN
        constraint_type := 'patch';
    ELSIF constraint_string ~ '^"\ *v?[0-9]+"$' THEN
        constraint_type := 'minor';
    ELSIF constraint_string ~ '^"\ *v?[0-9]+\.[0-9]+\.[0-9]+-?.*"$' THEN
        constraint_type := '='; -- essentially a `=`
    ELSIF constraint_string LIKE '"*"' 
      OR constraint_string LIKE '" *"' 
      OR constraint_string LIKE '""' 
      OR constraint_string LIKE '"x"' 
      OR constraint_string LIKE '" x"'
      OR constraint_string LIKE '"x.*"'
      OR constraint_string LIKE '" x.*"'
      OR constraint_string LIKE '"x.x"'
      OR constraint_string LIKE '" x.x"' THEN 
        constraint_type := 'major';
    ELSE
        constraint_type := 'invalid';
    END IF;
    RETURN constraint_type;
END;
$$ LANGUAGE plpgsql;


CREATE TABLE metadata_analysis.constraint_types AS
SELECT 
    id as dependency_id, 
    CASE WHEN (spec).dep_type = 'range' AND 
              jsonb_typeof(raw_spec) = 'string' 
              THEN metadata_analysis.check_constraint_type(raw_spec::text)
         ELSE NULL
    END as constraint_type
FROM dependencies;



CREATE INDEX constraint_types_idx ON metadata_analysis.constraint_types (dependency_id);

ANALYZE metadata_analysis.constraint_types;

GRANT SELECT ON metadata_analysis.constraint_types TO data_analyzer;
GRANT ALL ON metadata_analysis.constraint_types TO pinckney;
GRANT ALL ON metadata_analysis.constraint_types TO federico;


    