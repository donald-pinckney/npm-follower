CREATE OR REPLACE FUNCTION metadata_analysis.get_install_scripts(jsonb) RETURNS jsonb AS $$
SELECT coalesce($1, '{}'::jsonb)
$$ LANGUAGE SQL IMMUTABLE;
-- SELECT ARRAY[$1->>'preinstall', $1->>'install', $1->>'postinstall', $1->>'prepublish', $1->>'preprepare', $1->>'prepare', $1->>'postprepare']


CREATE TABLE metadata_analysis.update_did_change_json_scripts AS
select 
	u.from_id, 
    u.to_id, 
    metadata_analysis.get_install_scripts(from_v.extra_metadata -> 'scripts') as from_scripts, 
    metadata_analysis.get_install_scripts(to_v.extra_metadata -> 'scripts') as to_scripts, 
    metadata_analysis.get_install_scripts(from_v.extra_metadata -> 'scripts') <>  metadata_analysis.get_install_scripts(to_v.extra_metadata -> 'scripts') as did_change_json_scripts
from metadata_analysis.all_updates u
inner join versions from_v on u.from_id = from_v.id
inner join versions to_v on u.to_id = to_v.id;
-- where metadata_analysis.get_install_scripts(from_v.extra_metadata -> 'scripts') <>  metadata_analysis.get_install_scripts(to_v.extra_metadata -> 'scripts')

GRANT SELECT ON metadata_analysis.update_did_change_json_scripts TO data_analyzer;
GRANT ALL ON metadata_analysis.update_did_change_json_scripts TO pinckney;
GRANT ALL ON metadata_analysis.update_did_change_json_scripts TO federico;

ALTER TABLE metadata_analysis.update_did_change_json_scripts
ADD PRIMARY KEY (from_id, to_id);

ANALYZE metadata_analysis.update_did_change_json_scripts;
