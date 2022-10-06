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
  latest_date                 DATE
);
