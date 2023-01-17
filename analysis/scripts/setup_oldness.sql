CREATE TYPE oldness_pair AS (
    old_secs BIGINT,
    dep_pkg_id BIGINT
);

CREATE TABLE historic_solver_job_results_oldnesses (
    update_from_id BIGINT,
    update_to_id BIGINT,
    downstream_package_id BIGINT,
    oldnesses oldness_pair[] NOT NULL,
    PRIMARY KEY(
        update_from_id,
        update_to_id,
        downstream_package_id
    )
);