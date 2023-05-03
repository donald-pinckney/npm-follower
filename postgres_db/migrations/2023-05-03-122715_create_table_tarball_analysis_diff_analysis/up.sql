CREATE TABLE tarball_analysis.diff_analysis (
  from_id BIGINT NOT NULL,
  to_id BIGINT NOT NULL,
  job_result JSONB NOT NULL,
  PRIMARY KEY (from_id, to_id)
)