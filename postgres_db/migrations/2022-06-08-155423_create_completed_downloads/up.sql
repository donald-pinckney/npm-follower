-- Your SQL goes here

CREATE TABLE downloaded_tarballs (
  tarball_url TEXT PRIMARY KEY NOT NULL,
  downloaded_at TIMESTAMP WITH TIME ZONE NOT NULL,

  shasum TEXT,
  unpacked_size BIGINT,
  file_count INTEGER,
  integrity TEXT,
  signature0_sig TEXT,
  signature0_keyid TEXT,
  npm_signature TEXT,

  tgz_local_path TEXT,
  blob_storage_key TEXT,

  num_bytes BIGINT,

  CONSTRAINT check_at_least_one_storage CHECK (
    tgz_local_path IS NOT NULL OR blob_storage_key IS NOT NULL
  )
);