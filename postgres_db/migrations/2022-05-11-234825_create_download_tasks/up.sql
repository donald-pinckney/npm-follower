-- Your SQL goes here

CREATE TABLE download_tasks (
  package VARCHAR(2048) NOT NULL,
  version VARCHAR(2048) NOT NULL,

  url VARCHAR(2048) NOT NULL,
  change_seq BIGINT NOT NULL,
  
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
  success BOOLEAN NOT NULL,
  
  PRIMARY KEY(package, version),
  FOREIGN KEY (change_seq) REFERENCES change_log(seq)
);