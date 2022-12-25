CREATE TABLE ghsa (
  id TEXT PRIMARY KEY NOT NULL, -- GHSA id
  severity TEXT NOT NULL, -- Severity of the vulnerability
  description TEXT NOT NULL, -- Description of the vulnerability
  summary TEXT NOT NULL, -- Summary of the vulnerability
  withdrawn_at TIMESTAMP WITH TIME ZONE, -- Date when the vulnerability was withdrawn (if any)
  published_at TIMESTAMP WITH TIME ZONE NOT NULL, -- Date when the vulnerability was published
  updated_at TIMESTAMP WITH TIME ZONE NOT NULL, -- Date when the vulnerability was last updated
  refs TEXT[] NOT NULL, -- References (URLS) to the vulnerability
  cvss_score real, -- CVSS score of the vulnerability
  cvss_vector TEXT, -- CVSS vector of the vulnerability
  packages TEXT[] NOT NULL, -- Packages affected by the vulnerability
  vulns JSONB NOT NULL -- Vulnerabilities (JSONB)
);
