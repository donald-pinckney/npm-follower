CREATE TABLE metadata_analysis.vuln_patch_updates AS with valid_vulns as (
    select v.*,
        pkg.id as package_id
    from vulnerabilities v
        inner join ghsa adv on v.ghsa_id = adv.id
        inner join packages pkg on pkg.name = v.package_name
    where adv.withdrawn_at is null
),
vuln_patch_updates as (
    select u.package_id,
        u.from_id,
        u.to_id,
        u.from_semver,
        u.to_semver,
        u.from_created,
        u.to_created,
        u.ty,
        v.ghsa_id as patched_ghsa
    from metadata_analysis.all_updates u
        inner join valid_vulns v on u.package_id = v.package_id
        and v.first_patched_version = u.to_semver
)
select *
from vuln_patch_updates;

GRANT SELECT ON metadata_analysis.vuln_patch_updates TO data_analyzer;
GRANT ALL ON metadata_analysis.vuln_patch_updates TO pinckney;
GRANT ALL ON metadata_analysis.vuln_patch_updates TO federico;
