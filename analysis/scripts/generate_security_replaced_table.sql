select id, package_id, semver into security_replaced_versions from versions where (semver).major = 0 and (semver).minor = 0 and (semver).bug = 1 and (semver).build IS NULL and array_length((semver).prerelease, 1) = 1 and ((semver).prerelease[1]).string_case = 'security' and current_version_state_type = 'normal';

create table possibly_malware_versions (package_id BIGINT, id BIGINT, tarball_url TEXT);
