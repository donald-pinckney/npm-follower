UPDATE solving_analysis.historic_solver_job_inputs
SET job_state = 'none', start_time = NULL, work_node = NULL
WHERE job_state <> 'none';

DELETE FROM historic_solver_job_results;
