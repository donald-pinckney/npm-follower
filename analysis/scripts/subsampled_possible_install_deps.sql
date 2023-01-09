CREATE TABLE analysis.subsampled_possible_install_deps AS WITH random_dep_ranks AS (
    select *,
        ROW_NUMBER() over (
            partition by depends_on_pkg
            order by random()
        ) as random_rank
    from analysis.possible_install_deps
)
select pkg,
    depends_on_pkg
from random_dep_ranks
where random_rank <= 50;

ALTER TABLE analysis.subsampled_possible_install_deps
ADD PRIMARY KEY (pkg, depends_on_pkg);

ALTER TABLE analysis.subsampled_possible_install_deps
ADD CONSTRAINT analysis_subsampled_possible_install_deps_fkey_pkg FOREIGN KEY (pkg) REFERENCES packages (id);

ALTER TABLE analysis.subsampled_possible_install_deps
ADD CONSTRAINT analysis_subsampled_possible_install_deps_fkey_depends_on_pkg FOREIGN KEY (depends_on_pkg) REFERENCES packages (id);

ANALYZE analysis.subsampled_possible_install_deps;

GRANT SELECT ON analysis.subsampled_possible_install_deps TO data_analyzer;
GRANT ALL ON analysis.subsampled_possible_install_deps TO pinckney;
GRANT ALL ON analysis.subsampled_possible_install_deps TO federico;