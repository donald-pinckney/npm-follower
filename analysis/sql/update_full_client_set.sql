CREATE OR REPLACE FUNCTION metadata_analysis.semver_lt(semver_struct, semver_struct) RETURNS bool AS $$
SELECT CASE
	-- left beta, right beta
  WHEN (($1).prerelease is not null or ($1).build is not null) and (($2).prerelease is not null or ($2).build is not null) THEN $1 < $2
  -- left beta, right NON beta
  WHEN ($1).prerelease is not null or ($1).build is not null THEN $1 < $2 OR (($1).major = ($2).major and ($1).minor = ($2).minor and ($1).bug = ($2).bug)
  -- left NON beta, right beta
  WHEN ($2).prerelease is not null or ($2).build is not null THEN $1 < ROW(($2).major, ($2).minor, ($2).bug, NULL, NULL)::semver_struct
  -- left NON beta, right NON beta
  ELSE $1 < $2
END $$ LANGUAGE SQL IMMUTABLE;


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


CREATE OR REPLACE FUNCTION metadata_analysis.version_comp_exclude_betas(semver_struct, version_comparator_struct) RETURNS bool AS $$
SELECT CASE
	WHEN ($1).prerelease is not null or ($1).build is not null THEN FALSE
  WHEN ($2).operator = '*' THEN TRUE
  WHEN ($2).operator = '=' THEN $1 = ($2).semver
  WHEN ($2).operator = '>' THEN metadata_analysis.semver_lt(($2).semver, $1)
  WHEN ($2).operator = '>=' THEN ($2).semver = $1 OR metadata_analysis.semver_lt(($2).semver, $1)
  WHEN ($2).operator = '<=' THEN ($2).semver = $1 OR metadata_analysis.semver_lt($1, ($2).semver)
  WHEN ($2).operator = '<' THEN metadata_analysis.semver_lt($1, ($2).semver)
  ELSE false
END $$ LANGUAGE SQL IMMUTABLE;



create temp table U as (
  WITH package_client_counts AS (
    select p.id as package_id, direct.x as direct_clients, trans.x as trans_clients
    from packages p
    inner join metadata_analysis.all_dep_counts direct 
    on p.id = direct.pkg and direct.count_type = 'num_direct_runtime_rev_deps'
    inner join metadata_analysis.all_dep_counts trans
    on p.id = trans.pkg and trans.count_type = 'num_transitive_runtime_rev_deps'
    where p.current_package_state_type = 'normal'
  ),

  -- Step 1 in notes
  L AS (
    (
      SELECT package_id from package_client_counts
      order by direct_clients desc
      limit(100)
    )
    UNION
    (
      SELECT package_id from package_client_counts
      order by trans_clients desc
      limit(100)
    )
  ),

  -- Step 2 in notes
  U_full AS (
    select L.package_id, up.from_id, up.to_id, up.from_created, up.to_created, up.ty from L
    join metadata_analysis.all_updates up
    on L.package_id = up.package_id and up.ty <> 'zero_to_something'
  )


  -- We also subsample down the updates. For each package, we keep:
  -- 1. All major updates
  -- 2. The most recent 5 minor updates
  -- 3. The most recent 5 bug updates
  -- This gives us about the same amount per category

  select package_id, from_id, to_id, from_created, to_created, ty from (
    select *, ROW_NUMBER() OVER (PARTITION BY package_id ORDER BY to_created desc) AS r from U_full where ty = 'bug'
  ) _t1 
  where r <= 5
  union all
  select package_id, from_id, to_id, from_created, to_created, ty from (
    select *, ROW_NUMBER() OVER (PARTITION BY package_id ORDER BY to_created desc) AS r from U_full where ty = 'minor'
  ) _t2 
  where r <= 5
  union all
  select * from U_full where ty = 'major'
)
;

CREATE INDEX ON U (package_id);
CREATE INDEX ON U (from_id);
CREATE INDEX ON U (to_created);
ANALYZE U;




create temp table dep_version_match_rel as (
  WITH
  disjunct_unnest AS (
    SELECT 
      *, 
      row_number() over (partition by dep_id) as disjunct_id
    FROM (
      SELECT dst_package_id_if_exists, id as dep_id, raw_spec, unnest((spec).range_disjuncts) as conjuncts FROM dependencies
      WHERE dst_package_id_if_exists IS NOT NULL AND dst_package_id_if_exists IN (SELECT package_id from U) AND (spec).dep_type = 'range' 
    ) t
  ),

  term_unnest AS (
    SELECT 
      dst_package_id_if_exists, dep_id, disjunct_id,
      unnest((conjuncts).conjuncts) as term 
    FROM disjunct_unnest
  ),

  conjunct_eval AS (
    SELECT t.dep_id, v.semver, v.id as v_id, bool_and(metadata_analysis.version_comp_exclude_betas(v.semver, t.term)) as conj
    FROM term_unnest t
    INNER JOIN versions v
    ON v.package_id = t.dst_package_id_if_exists
    GROUP BY t.dep_id, t.disjunct_id, v.id
  ),

  dep_eval_ranges AS (
    SELECT dep_id, v_id
    FROM conjunct_eval
    GROUP BY dep_id, v_id
    HAVING bool_or(conj)
  ),

  dep_eval_tag_latest AS (
    SELECT d.id as dep_id, v.id as v_id
    FROM dependencies d
    INNER JOIN versions v
    ON 
          d.dst_package_id_if_exists IS NOT NULL 
      AND d.dst_package_id_if_exists IN (SELECT package_id from U)
      AND d.dst_package_id_if_exists = v.package_id 
      AND (v.semver).prerelease IS NULL 
      AND (v.semver).build IS NULL 
      AND (d.spec).dep_type = 'tag' AND (d.spec).tag_name = 'latest'
  )

  SELECT * FROM dep_eval_ranges
  UNION ALL
  SELECT * FROM dep_eval_tag_latest
);

CREATE INDEX ON dep_version_match_rel (dep_id);
CREATE INDEX ON dep_version_match_rel (v_id);
ANALYZE dep_version_match_rel;


create table metadata_analysis.update_full_client_set AS (
  select 
  distinct on (client.package_id)
  dep_match_v.dep_id, dep_match_v.v_id as lib_v, up.package_id as lib_p, up.to_id as lib_v2, up.from_created as lib_v_t, up.to_created as lib_v2_t, client.id as cli_v, client.package_id as cli_p, client.semver as cli_sem, client.created as cli_t, ROW(dep_match_v.dep_id, up.to_id) IN (SELECT * from dep_version_match_rel) as auto_update
  from dep_version_match_rel dep_match_v
  inner join U up
  on up.from_id = dep_match_v.v_id
  inner join metadata_analysis.version_unnest_prod_dependencies v
  on dep_match_v.dep_id = v.prod_dep_id
  inner join versions client
  on client.id = v.version_id and client.created < up.to_created and (client.semver).prerelease is null and (client.semver).build is null 
  order by client.package_id, client.created DESC
);


