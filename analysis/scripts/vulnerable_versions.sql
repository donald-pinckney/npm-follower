CREATE OR REPLACE FUNCTION analysis.non_beta_semver(semver) RETURNS semver AS $$
SELECT ROW(($1).major, ($1).minor, ($1).bug, NULL, NULL)::semver $$ LANGUAGE SQL IMMUTABLE;




CREATE OR REPLACE FUNCTION analysis.less_than_semver(semver_struct, semver_struct, bool) RETURNS bool AS $$
SELECT CASE
        WHEN $1 IS NULL
        OR $2 IS NULL THEN TRUE
        WHEN $3 THEN $1 <= $2
        ELSE $1 < $2
    END $$ LANGUAGE SQL IMMUTABLE;


CREATE OR REPLACE FUNCTION analysis.matches_range_non_beta(semver, semver_struct, bool, semver_struct, bool) RETURNS bool AS $$
SELECT analysis.less_than_semver($2, $1, $3)
    AND analysis.less_than_semver($1, $4, $5)
END $$ LANGUAGE SQL IMMUTABLE;

            
    
CREATE OR REPLACE FUNCTION analysis.matches_range(semver, semver_struct, bool, semver_struct, bool) RETURNS bool AS $$
SELECT CASE
        WHEN NOT $2 IS NULL
        AND (
            ($2).prerelease IS NOT NULL
            OR ($2).build IS NOT NULL
        )
        AND NOT $4 IS NULL
        AND (
            ($4).prerelease IS NOT NULL
            OR ($4).build IS NOT NULL
        ) THEN analysis.matches_range_non_beta(
            $1,
            analysis.non_beta_semver($2),
            TRUE,
            analysis.non_beta_semver($4),
            FALSE
        )
        WHEN NOT $2 IS NULL
        AND (
            ($2).prerelease IS NOT NULL
            OR ($2).build IS NOT NULL
        ) THEN analysis.matches_range_non_beta($1, analysis.non_beta_semver($2), TRUE, $4, $5)
        WHEN NOT $4 IS NULL
        AND (
            ($4).prerelease IS NOT NULL
            OR ($4).build IS NOT NULL
        ) THEN analysis.matches_range_non_beta($1, $2, $3, analysis.non_beta_semver($4), FALSE)
        ELSE analysis.matches_range_non_beta($1, $2, $3, $4, $5)
    END $$ LANGUAGE SQL IMMUTABLE;


CREATE TABLE analysis.vulnerable_versions AS
select vers.semver,
    vuln.id as vuln_id
from versions vers
    inner join packages pkg on vers.package_id = pkg.id
    and (vers.semver).prerelease IS NULL
    and (vers.semver).build IS NULL
    inner join vulnerabilities vuln on vuln.package_name = pkg.name
    and analysis.matches_range(
        vers.semver,
        vuln.vulnerable_version_lower_bound,
        vuln.vulnerable_version_lower_bound_inclusive,
        vuln.vulnerable_version_upper_bound,
        vuln.vulnerable_version_upper_bound_inclusive
    );


GRANT SELECT ON analysis.vulnerable_versions TO data_analyzer;
