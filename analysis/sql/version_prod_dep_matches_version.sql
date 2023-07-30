CREATE
OR REPLACE FUNCTION metadata_analysis.semver_lt(semver_struct, semver_struct) RETURNS bool AS $ $
SELECT
    CASE
        -- left beta, right beta
        WHEN (
            ($ 1).prerelease is not null
            or ($ 1).build is not null
        )
        and (
            ($ 2).prerelease is not null
            or ($ 2).build is not null
        ) THEN $ 1 < $ 2 -- left beta, right NON beta
        WHEN ($ 1).prerelease is not null
        or ($ 1).build is not null THEN $ 1 < $ 2
        OR (
            ($ 1).major = ($ 2).major
            and ($ 1).minor = ($ 2).minor
            and ($ 1).bug = ($ 2).bug
        ) -- left NON beta, right beta
        WHEN ($ 2).prerelease is not null
        or ($ 2).build is not null THEN $ 1 < ROW(($ 2).major, ($ 2).minor, ($ 2).bug, NULL, NULL) :: semver_struct -- left NON beta, right NON beta
        ELSE $ 1 < $ 2
    END $ $ LANGUAGE SQL IMMUTABLE;

-- SELECT *, metadata_analysis.semver_lt(a, b) FROM (VALUES 
-- 	(ROW(1, 2, 3, NULL, NULL)::semver_struct, ROW(1, 2, 3, NULL, NULL)::semver_struct),
--   (ROW(1, 2, 3, ARRAY[ROW('int', NULL, 5)::prerelease_tag_struct], ARRAY['a'])::semver_struct, ROW(1, 2, 3, NULL, NULL)::semver_struct),
--   (ROW(1, 2, 3, NULL, ARRAY['a'])::semver_struct, ROW(1, 2, 3, NULL, ARRAY['b'])::semver_struct),
--   (ROW(1, 2, 3, NULL, ARRAY['c'])::semver_struct, ROW(1, 2, 3, NULL, ARRAY['b'])::semver_struct),
-- 	(ROW(1, 2, 3, ARRAY[ROW('int', NULL, 5)::prerelease_tag_struct], NULL)::semver_struct, ROW(1, 2, 3, ARRAY[ROW('string', 'foo', NULL)::prerelease_tag_struct], NULL)::semver_struct),    
--   (ROW(1, 2, 3, NULL, ARRAY['c'])::semver_struct, ROW(1, 2, 3, NULL, NULL)::semver_struct),
--   (ROW(1, 3, 3, NULL, ARRAY['c'])::semver_struct, ROW(1, 2, 3, NULL, NULL)::semver_struct),
--   (ROW(1, 0, 3, NULL, ARRAY['c'])::semver_struct, ROW(1, 2, 3, NULL, NULL)::semver_struct),
--   (ROW(1, 2, 3, NULL, NULL)::semver_struct, ROW(1, 2, 3, NULL, ARRAY['c'])::semver_struct)                                                
-- ) AS t (a, b);
CREATE
OR REPLACE FUNCTION metadata_analysis.version_comp_exclude_betas(semver_struct, version_comparator_struct) RETURNS bool AS $ $
SELECT
    CASE
        WHEN ($ 1).prerelease is not null
        or ($ 1).build is not null THEN FALSE
        WHEN ($ 2).operator = '*' THEN TRUE
        WHEN ($ 2).operator = '=' THEN $ 1 = ($ 2).semver
        WHEN ($ 2).operator = '>' THEN metadata_analysis.semver_lt(($ 2).semver, $ 1)
        WHEN ($ 2).operator = '>=' THEN ($ 2).semver = $ 1
        OR metadata_analysis.semver_lt(($ 2).semver, $ 1)
        WHEN ($ 2).operator = '<=' THEN ($ 2).semver = $ 1
        OR metadata_analysis.semver_lt($ 1, ($ 2).semver)
        WHEN ($ 2).operator = '<' THEN metadata_analysis.semver_lt($ 1, ($ 2).semver)
        ELSE false
    END $ $ LANGUAGE SQL IMMUTABLE;

---------
CREATE
OR REPLACE VIEW metadata_analysis.version_prod_dep_matches_version AS WITH disjunct_unnest AS (
    SELECT
        *,
        row_number() over (partition by dep_id) as disjunct_id
    FROM
        (
            SELECT
                dst_package_id_if_exists,
                id as dep_id,
                raw_spec,
                unnest((spec).range_disjuncts) as conjuncts
            FROM
                dependencies
            WHERE
                dst_package_id_if_exists IS NOT NULL
                AND (spec).dep_type = 'range'
        ) t
),
term_unnest AS (
    SELECT
        dst_package_id_if_exists,
        dep_id,
        disjunct_id,
        unnest((conjuncts).conjuncts) as term
    FROM
        disjunct_unnest
),
conjunct_eval AS (
    SELECT
        t.dep_id,
        v.semver,
        v.id as v_id,
        bool_and(
            metadata_analysis.version_comp_exclude_betas(v.semver, t.term)
        ) as conj
    FROM
        term_unnest t
        INNER JOIN versions v ON v.package_id = t.dst_package_id_if_exists --AND metadata_analysis.version_comp_exclude_betas(v.semver, t.term)
    GROUP BY
        t.dep_id,
        t.disjunct_id,
        v.id
),
dep_eval_ranges AS (
    SELECT
        dep_id,
        v_id
    FROM
        conjunct_eval
    GROUP BY
        dep_id,
        v_id
    HAVING
        bool_or(conj)
),
dep_eval_tag_latest AS (
    SELECT
        d.id as dep_id,
        v.id as v_id
    FROM
        dependencies d
        INNER JOIN versions v ON d.dst_package_id_if_exists IS NOT NULL
        AND d.dst_package_id_if_exists = v.package_id
        AND (v.semver).prerelease IS NULL
        AND (v.semver).build IS NULL
        AND (d.spec).dep_type = 'tag'
        AND (d.spec).tag_name = 'latest'
),
dep_version_match_rel AS (
    SELECT
        *
    FROM
        dep_eval_ranges
    UNION
    ALL
    SELECT
        *
    FROM
        dep_eval_tag_latest
)
SELECT
    v.id as src_v,
    dep_match_v.v_id as dst_v
FROM
    (
        SELECT
            id,
            unnest(prod_dependencies) as src_dep_id
        from
            versions
    ) v
    INNER JOIN dep_version_match_rel dep_match_v ON v.src_dep_id = dep_match_v.dep_id;

-- SELECT
--     *
-- FROM
--     metadata_analysis.version_prod_dep_matches_version
-- WHERE
--     dst_v = 19989635