-- Your SQL goes here

CREATE TABLE downloaded_tarballs (
  tarball_url TEXT NOT NULL,
  downloaded_at TIMESTAMP WITH TIME ZONE NOT NULL,

  shasum TEXT,
  unpacked_size BIGINT,
  file_count INTEGER,
  integrity TEXT,
  signature0_sig TEXT,
  signature0_keyid TEXT,
  npm_signature TEXT,

  tgz_local_path TEXT NOT NULL,

  PRIMARY KEY(tarball_url, downloaded_at)
);