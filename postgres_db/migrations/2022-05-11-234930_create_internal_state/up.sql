-- Your SQL goes here

CREATE TABLE internal_state (
  key VARCHAR(127) PRIMARY KEY NOT NULL,
  int_value BIGINT,
  string_value TEXT
);
