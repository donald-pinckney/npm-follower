UPDATE historic_solver.job_inputs
SET job_state = 'none'
WHERE job_state = 'started'
    AND ROW(
        update_from_id,
        update_to_id,
        downstream_package_id
    ) NOT IN (
        SELECT update_from_id,
            update_to_id,
            downstream_package_id
        FROM historic_solver.job_results
    );