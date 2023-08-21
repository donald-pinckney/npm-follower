CREATE TABLE metadata_analysis.total_package_downloads AS

with unnested_counts as (
  select package_id, (unnest(download_counts)).counter as total_downloads from download_metrics
)

select package_id, sum(total_downloads)
from unnested_counts
group by package_id
;

ALTER TABLE metadata_analysis.total_package_downloads
ADD PRIMARY KEY (package_id);

ANALYZE metadata_analysis.total_package_downloads;

GRANT SELECT ON metadata_analysis.total_package_downloads TO data_analyzer;
GRANT ALL ON metadata_analysis.total_package_downloads TO pinckney;
GRANT ALL ON metadata_analysis.total_package_downloads TO federico;