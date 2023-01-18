CREATE TABLE analysis.vuln_intro_updates AS with valid_vulns as (
    select v.*,
        pkg.id as package_id
    from vulnerabilities v
        inner join ghsa adv on v.ghsa_id = adv.id
        inner join packages pkg on pkg.name = v.package_name
    where adv.withdrawn_at is null
),
vuln_vers_ordering as (
    select vuln_id,
        semver,
        ROW_NUMBER() over (
            partition by vuln_id
            order by semver
        ) as smaller_vers_order
    from analysis.vulnerable_versions
)
select u.package_id,
    u.from_id,
    u.to_id,
    u.from_semver,
    u.to_semver,
    u.from_created,
    u.to_created,
    u.ty,
    v.ghsa_id as introduced_ghsa
from analysis.all_updates u
    inner join valid_vulns v on u.package_id = v.package_id
    inner join vuln_vers_ordering v_o ON v_o.vuln_id = v.id
    AND v_o.semver = u.to_semver
    AND v_o.smaller_vers_order = 1;
GRANT SELECT ON analysis.vuln_intro_updates TO data_analyzer;
