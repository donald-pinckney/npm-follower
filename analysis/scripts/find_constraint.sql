-- this is a psql script.

-- function that given a constraint string (e.g. "^1.2.3"), gives the contraint type ("^")
-- there are 8 types of constraints: ^, ~, <, >, =, <=, >=, and *. we will check if 
-- the string contains either of them, and return the first one we find. if none are found,
-- we return NULL.
-- also, we have `A || B`, `A, B`, `A - B`, and others.
CREATE OR REPLACE FUNCTION find_constraint(constraint_string text) RETURNS text AS $$
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
