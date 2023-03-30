UPDATE solving_analysis.historic_solver_job_inputs
SET job_state = 'none', start_time = NULL, work_node = NULL
WHERE job_state = 'done'
    AND ROW(
        update_from_id,
        update_to_id,
        downstream_package_id,
        'FromMissing'
    ) IN (
        SELECT update_from_id,
            update_to_id,
            downstream_package_id,
            result_category
        FROM historic_solver_job_results
    );

DELETE FROM historic_solver_job_results WHERE result_category = 'FromMissing';