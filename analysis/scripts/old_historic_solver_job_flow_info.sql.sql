CREATE TABLE analysis.old_historic_solver_job_flow_info AS
select 
  array_length(solve_history, 1) = 2 as instant_update,
  array_length(solve_history, 1) - 2 as update_days,
  (solve_history[array_length(solve_history, 1)]).downstream_version <> (solve_history[1]).downstream_version as downstream_updated_req,
  ROW(update_from_id, update_to_id) IN (select from_id, to_id from analysis.vuln_intro_updates) as is_intro,
  ROW(update_from_id, update_to_id) IN (select from_id, to_id from analysis.vuln_patch_updates) and ROW(update_from_id, update_to_id) NOT IN (select from_id, to_id from analysis.vuln_intro_updates) as is_patch,
  ROW(update_from_id, update_to_id) NOT IN (select from_id, to_id from analysis.vuln_patch_updates) and ROW(update_from_id, update_to_id) NOT IN (select from_id, to_id from analysis.vuln_intro_updates) as is_neutral,
  *
from old_historic_solver_job_results

where result_category <> 'SolveError' 
and result_category <> 'DownstreamMissingVersion'
and result_category <> 'DownstreamMissingPackage' 
and result_category <> 'FromMissingVersion' 
and result_category <> 'FromMissingPackage'
and result_category <> 'MiscError'
-- GaveUp, RemovedDep, Ok
;


GRANT SELECT ON analysis.old_historic_solver_job_flow_info TO data_analyzer;

