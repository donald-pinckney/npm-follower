CREATE TABLE metadata_analysis.all_updates AS WITH intra_group_updates AS (
    SELECT from_v.package_id AS package_id,
        from_v.group_base_semver AS from_group_base_semver,
        to_v.group_base_semver AS to_group_base_semver,
        from_v.id AS from_id,
        to_v.id AS to_id,
        from_v.semver AS from_semver,
        to_v.semver AS to_semver,
        from_v.created AS from_created,
        to_v.created AS to_created,
        metadata_analysis.determine_update_type(from_v.semver, to_v.semver) AS ty
    FROM metadata_analysis.valid_non_betas_with_ordering from_v
        INNER JOIN metadata_analysis.valid_non_betas_with_ordering to_v ON from_v.package_id = to_v.package_id
        AND from_v.group_base_semver = to_v.group_base_semver
        AND from_v.order_within_group + 1 = to_v.order_within_group
),
selected_inter_group_predecessors AS (
    SELECT from_v.package_id AS package_id,
        from_v.group_base_semver AS group_base_semver,
        MAX(from_v.chron_order_global) AS greatest_lower_chron_order_global,
        to_v.start_id AS to_id
    FROM metadata_analysis.valid_non_betas_with_ordering from_v
        INNER JOIN metadata_analysis.valid_group_ranges to_v ON from_v.package_id = to_v.package_id
        AND from_v.inter_group_order + 1 = to_v.inter_group_order
        AND from_v.chron_order_global < to_v.start_chron_order_global
    GROUP BY from_v.package_id,
        from_v.group_base_semver,
        to_v.start_id -- we don't actually group by to_v.start_id, since it is unique per (from_v.package_id, from_v.group_base_semver),
        -- but its necessary to put in the GROUP BY to include in the SELECT
),
inter_group_updates AS (
    SELECT from_v.package_id AS package_id,
        from_v.group_base_semver AS from_group_base_semver,
        to_v.group_base_semver AS to_group_base_semver,
        from_v.id AS from_id,
        to_v.id AS to_id,
        from_v.semver AS from_semver,
        to_v.semver AS to_semver,
        from_v.created AS from_created,
        to_v.created AS to_created,
        metadata_analysis.determine_update_type(from_v.semver, to_v.semver) AS ty
    FROM selected_inter_group_predecessors preds
        INNER JOIN metadata_analysis.valid_non_betas_with_ordering from_v ON from_v.package_id = preds.package_id
        AND from_v.chron_order_global = preds.greatest_lower_chron_order_global
        INNER JOIN metadata_analysis.valid_non_betas_with_ordering to_v ON to_v.id = preds.to_id
)
SELECT *
FROM intra_group_updates
UNION ALL
SELECT *
FROM inter_group_updates;


CREATE INDEX metadata_analysis_all_updates_idx_package_id ON metadata_analysis.all_updates (package_id);
CREATE INDEX metadata_analysis_all_updates_idx_to_semver ON metadata_analysis.all_updates (to_semver);
CREATE INDEX metadata_analysis_all_updates_idx_to_id ON metadata_analysis.all_updates (to_id);
CREATE INDEX metadata_analysis_all_updates_idx_from_id ON metadata_analysis.all_updates (from_id);

ALTER TABLE metadata_analysis.all_updates
ADD PRIMARY KEY (from_id, to_id);

ANALYZE metadata_analysis.all_updates;

GRANT SELECT ON metadata_analysis.all_updates TO data_analyzer;
GRANT ALL ON metadata_analysis.all_updates TO pinckney;
GRANT ALL ON metadata_analysis.all_updates TO federico;


CREATE TABLE metadata_analysis.all_overlaps AS
SELECT x.package_id AS package_id,
    x.group_base_semver AS first_group_base_semver,
    y.group_base_semver AS second_group_base_semver,
    x.start_created AS first_group_start_created,
    x.end_created AS first_group_end_created,
    y.start_created AS second_group_start_created,
    y.end_created AS second_group_end_created
FROM metadata_analysis.valid_group_ranges x
    INNER JOIN metadata_analysis.valid_group_ranges y ON x.package_id = y.package_id
    AND x.inter_group_order < y.inter_group_order
    AND x.end_created >= y.start_created;

GRANT SELECT ON metadata_analysis.all_overlaps TO data_analyzer;
GRANT ALL ON metadata_analysis.all_overlaps TO pinckney;
GRANT ALL ON metadata_analysis.all_overlaps TO federico;