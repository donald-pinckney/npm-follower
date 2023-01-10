CREATE TYPE historic_solver_solve_result_struct AS (
    solve_time TIMESTAMP WITH TIME ZONE,
    downstream_version semver_struct,
    present_versions semver_struct []
);


CREATE TABLE historic_solver_job_inputs (
    update_from_id BIGINT,
    update_to_id BIGINT,
    downstream_package_id BIGINT,
    job_state TEXT NOT NULL,
    -- ("none", "started", "done")
    start_time TIMESTAMP WITH TIME ZONE,
    work_node TEXT,
    update_package_name TEXT NOT NULL,
    update_from_version semver NOT NULL,
    update_to_version semver NOT NULL,
    update_to_time TIMESTAMP WITH TIME ZONE NOT NULL,
    downstream_package_name TEXT NOT NULL,
    PRIMARY KEY (
        update_from_id,
        update_to_id,
        downstream_package_id
    )
);

CREATE TABLE historic_solver_job_results (
    update_from_id BIGINT,
    update_to_id BIGINT,
    downstream_package_id BIGINT,
    result_category TEXT NOT NULL,
    solve_history historic_solver_solve_result_struct [] NOT NULL,
    stdout TEXT NOT NULL,
    stderr TEXT NOT NULL,
    -- [(solve_time, [v])]
    PRIMARY KEY(
        update_from_id,
        update_to_id,
        downstream_package_id
    )
);

CREATE INDEX state_idx ON historic_solver_job_inputs (job_state);


GRANT ALL ON historic_solver_job_inputs TO historic_solve_runner;
GRANT ALL ON historic_solver_job_results TO historic_solve_runner;

GRANT ALL ON TYPE historic_solver_solve_result_struct TO historic_solve_runner;
GRANT ALL ON TYPE semver_struct TO historic_solve_runner;
GRANT USAGE ON SCHEMA public TO historic_solve_runner;

GRANT SELECT ON historic_solver_job_inputs TO data_analyzer;
GRANT SELECT ON historic_solver_job_results TO data_analyzer;