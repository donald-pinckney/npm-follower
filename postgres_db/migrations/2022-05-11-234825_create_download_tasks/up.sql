-- Your SQL goes here

CREATE TABLE download_tasks (
  url VARCHAR(2048) PRIMARY KEY NOT NULL,
  
  shasum TEXT,
  unpacked_size BIGINT,
  file_count INTEGER,
  integrity TEXT,
  signature0_sig TEXT,
  signature0_keyid TEXT,
  npm_signature TEXT,

  queue_time TIMESTAMP WITH TIME ZONE NOT NULL,
  num_failures INTEGER NOT NULL,
  last_failure TIMESTAMP WITH TIME ZONE,
  failed BOOLEAN -- This can be: NULL (not attempted / in progress), FALSE (succeeded), TRUE (failed)
);