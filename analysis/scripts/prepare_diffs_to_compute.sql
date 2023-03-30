CREATE TEMP TABLE updates_with_urls AS
SELECT u.from_id AS from_id,
    u.to_id AS to_id,
    from_v.tarball_url AS from_url,
    to_v.tarball_url AS to_url
FROM analysis.all_updates u
    INNER JOIN versions from_v ON u.from_id = from_v.id
    INNER JOIN versions to_v ON u.to_id = to_v.id;

CREATE TABLE analysis.diffs_to_compute AS
SELECT u.*,
    from_tar.blob_storage_key AS from_key,
    to_tar.blob_storage_key AS to_key
FROM updates_with_urls u
    INNER JOIN downloaded_tarballs from_tar ON u.from_url = from_tar.tarball_url
    INNER JOIN downloaded_tarballs to_tar ON u.to_url = to_tar.tarball_url;

ALTER TABLE analysis.diffs_to_compute
ADD PRIMARY KEY (from_id, to_id);

DELETE FROM analysis.diffs_to_compute WHERE ROW(from_id, to_id) IN (SELECT from_id, to_id FROM tarball_analysis.diff_analysis);

GRANT ALL ON analysis.diffs_to_compute TO pinckney;
GRANT ALL ON analysis.diffs_to_compute TO federico;
GRANT SELECT ON analysis.diffs_to_compute TO data_analyzer;

ANALYZE analysis.diffs_to_compute;