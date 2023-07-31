CREATE TABLE metadata_analysis.package_avg_sizes AS
select p.id as package_id, avg(t.unpacked_size) as avg_code_size
    from packages p
    inner join versions v on v.package_id = p.id and v.current_version_state_type = 'normal'
    inner join downloaded_tarballs t on t.tarball_url = v.tarball_url
    where p.current_package_state_type = 'normal'
    group by p.id;
  
ALTER TABLE metadata_analysis.package_avg_sizes
ADD PRIMARY KEY (package_id);

ANALYZE metadata_analysis.package_avg_sizes;