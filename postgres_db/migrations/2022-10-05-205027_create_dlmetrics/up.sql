-- Your SQL goes here

CREATE TYPE download_count_struct AS ( 
    time DATE,
    counter BIGINT
);

CREATE TABLE download_metrics (
  id                          BIGINT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
  package_id                  BIGINT NOT NULL UNIQUE,
  -- here we have an array of (timestamp, count) tuples
  download_counts             download_count_struct[] NOT NULL,
  total_downloads             BIGINT NOT NULL,
  latest_date                 DATE NOT NULL
);

-- we want to index dates where older dates are more likely to be queried
CREATE INDEX download_metrics_latest_date ON download_metrics (latest_date DESC) WHERE
latest_date > '2022-01-01';


