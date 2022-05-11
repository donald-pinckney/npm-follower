-- Your SQL goes here

CREATE TABLE change_log (
  seq BIGINT PRIMARY KEY NOT NULL,
  raw_json JSONB NOT NULL
);
