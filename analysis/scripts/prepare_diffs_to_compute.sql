CREATE TEMP TABLE updates_with_urls AS
SELECT u.from_id AS from_id,
    u.to_id AS to_id,
    from_v.tarball_url AS from_url,
    to_v.tarball_url AS to_url
FROM analysis.all_updates u
    INNER JOIN versions from_v ON u.from_id = from_v.id
    INNER JOIN versions to_v ON u.to_id = to_v.id;

CREATE UNLOGGED TABLE analysis.diffs_to_compute AS
SELECT u.*,
    from_tar.blob_storage_key AS from_key,
    to_tar.blob_storage_key AS to_key
FROM updates_with_urls u
    INNER JOIN downloaded_tarballs from_tar ON u.from_url = from_tar.tarball_url
    INNER JOIN downloaded_tarballs to_tar ON u.to_url = to_tar.tarball_url;

ALTER TABLE analysis.diffs_to_compute ADD PRIMARY KEY (from_id, to_id);

GRANT ALL ON analysis.diffs_to_compute TO pinckney;
GRANT ALL ON analysis.diffs_to_compute TO federico;

ANALYZE analysis.diffs_to_compute;