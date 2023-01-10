INSERT INTO historic_solver_job_inputs
SELECT u.from_id AS update_from_id,
    u.to_id AS update_to_id,
    d.pkg AS downstream_package_id,
    'none' AS job_state,
    NULL AS start_time,
    NULL AS work_node,
    up.name AS update_package_name,
    u.from_semver AS update_from_version,
    u.to_semver AS update_to_version,
    u.to_created AS update_to_time,
    dp.name AS downstream_package_name
FROM analysis.subsampled_updates u
    INNER JOIN analysis.subsampled_possible_install_deps d ON u.package_id = d.depends_on_pkg
    INNER JOIN packages up ON up.id = u.package_id
    INNER JOIN packages dp ON dp.id = d.pkg;