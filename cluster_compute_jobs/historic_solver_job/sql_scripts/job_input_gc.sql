UPDATE solving_analysis.historic_solver_job_inputs
SET job_state = 'done'
WHERE job_state = 'started'
    AND ROW(
        update_from_id,
        update_to_id,
        downstream_package_id
    ) IN (
        SELECT update_from_id,
            update_to_id,
            downstream_package_id
        FROM historic_solver_job_results
    );