.PHONY: all
all: build_updates constraint_types deps_stats find_patches possible_direct_dev_deps possible_direct_runtime_deps possible_install_deps possible_transitive_runtime_deps possible_version_direct_runtime_deps prepare_diffs_to_compute security_replaced_versions setup_analysis subsampled_possible_install_deps subsampled_updates unique_package_deps version_ordering_validation vuln_intro_updates vulnerable_versions

# -------- build_updates --------
makefile_state/build_updates.touch: makefile_state/version_ordering_validation.touch
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/build_updates.sql
	touch makefile_state/build_updates.touch

.PHONY: build_updates
build_updates: makefile_state/build_updates.touch

.PHONY: clean_build_updates
clean_build_updates: clean_find_patches clean_prepare_diffs_to_compute clean_subsampled_updates clean_vuln_intro_updates
	if [ -f makefile_state/build_updates.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/build_updates.sql; else true; fi
	rm -f makefile_state/build_updates.touch

# -------- constraint_types --------
makefile_state/constraint_types.touch: makefile_state/setup_analysis.touch
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/constraint_types.sql
	touch makefile_state/constraint_types.touch

.PHONY: constraint_types
constraint_types: makefile_state/constraint_types.touch

.PHONY: clean_constraint_types
clean_constraint_types: clean_unique_package_deps
	if [ -f makefile_state/constraint_types.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/constraint_types.sql; else true; fi
	rm -f makefile_state/constraint_types.touch

# -------- deps_stats --------
makefile_state/deps_stats.touch: makefile_state/possible_direct_dev_deps.touch makefile_state/possible_direct_runtime_deps.touch makefile_state/possible_install_deps.touch makefile_state/possible_transitive_runtime_deps.touch
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/deps_stats.sql
	touch makefile_state/deps_stats.touch

.PHONY: deps_stats
deps_stats: makefile_state/deps_stats.touch

.PHONY: clean_deps_stats
clean_deps_stats: 
	if [ -f makefile_state/deps_stats.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/deps_stats.sql; else true; fi
	rm -f makefile_state/deps_stats.touch

# -------- find_patches --------
makefile_state/find_patches.touch: makefile_state/build_updates.touch
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/find_patches.sql
	touch makefile_state/find_patches.touch

.PHONY: find_patches
find_patches: makefile_state/find_patches.touch

.PHONY: clean_find_patches
clean_find_patches: 
	if [ -f makefile_state/find_patches.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/find_patches.sql; else true; fi
	rm -f makefile_state/find_patches.touch

# -------- possible_direct_dev_deps --------
makefile_state/possible_direct_dev_deps.touch: makefile_state/setup_analysis.touch
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/possible_direct_dev_deps.sql
	touch makefile_state/possible_direct_dev_deps.touch

.PHONY: possible_direct_dev_deps
possible_direct_dev_deps: makefile_state/possible_direct_dev_deps.touch

.PHONY: clean_possible_direct_dev_deps
clean_possible_direct_dev_deps: clean_deps_stats clean_possible_install_deps
	if [ -f makefile_state/possible_direct_dev_deps.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/possible_direct_dev_deps.sql; else true; fi
	rm -f makefile_state/possible_direct_dev_deps.touch

# -------- possible_direct_runtime_deps --------
makefile_state/possible_direct_runtime_deps.touch: makefile_state/setup_analysis.touch
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/possible_direct_runtime_deps.sql
	touch makefile_state/possible_direct_runtime_deps.touch

.PHONY: possible_direct_runtime_deps
possible_direct_runtime_deps: makefile_state/possible_direct_runtime_deps.touch

.PHONY: clean_possible_direct_runtime_deps
clean_possible_direct_runtime_deps: clean_deps_stats clean_possible_install_deps clean_possible_transitive_runtime_deps
	if [ -f makefile_state/possible_direct_runtime_deps.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/possible_direct_runtime_deps.sql; else true; fi
	rm -f makefile_state/possible_direct_runtime_deps.touch

# -------- possible_install_deps --------
makefile_state/possible_install_deps.touch: makefile_state/possible_direct_dev_deps.touch makefile_state/possible_direct_runtime_deps.touch makefile_state/possible_transitive_runtime_deps.touch
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/possible_install_deps.sql
	touch makefile_state/possible_install_deps.touch

.PHONY: possible_install_deps
possible_install_deps: makefile_state/possible_install_deps.touch

.PHONY: clean_possible_install_deps
clean_possible_install_deps: clean_deps_stats clean_subsampled_possible_install_deps
	if [ -f makefile_state/possible_install_deps.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/possible_install_deps.sql; else true; fi
	rm -f makefile_state/possible_install_deps.touch

# -------- possible_transitive_runtime_deps --------
makefile_state/possible_transitive_runtime_deps.touch: makefile_state/possible_direct_runtime_deps.touch
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/possible_transitive_runtime_deps.sql
	touch makefile_state/possible_transitive_runtime_deps.touch

.PHONY: possible_transitive_runtime_deps
possible_transitive_runtime_deps: makefile_state/possible_transitive_runtime_deps.touch

.PHONY: clean_possible_transitive_runtime_deps
clean_possible_transitive_runtime_deps: clean_deps_stats clean_possible_install_deps
	if [ -f makefile_state/possible_transitive_runtime_deps.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/possible_transitive_runtime_deps.sql; else true; fi
	rm -f makefile_state/possible_transitive_runtime_deps.touch

# -------- possible_version_direct_runtime_deps --------
makefile_state/possible_version_direct_runtime_deps.touch: makefile_state/setup_analysis.touch
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/possible_version_direct_runtime_deps.sql
	touch makefile_state/possible_version_direct_runtime_deps.touch

.PHONY: possible_version_direct_runtime_deps
possible_version_direct_runtime_deps: makefile_state/possible_version_direct_runtime_deps.touch

.PHONY: clean_possible_version_direct_runtime_deps
clean_possible_version_direct_runtime_deps: 
	if [ -f makefile_state/possible_version_direct_runtime_deps.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/possible_version_direct_runtime_deps.sql; else true; fi
	rm -f makefile_state/possible_version_direct_runtime_deps.touch

# -------- prepare_diffs_to_compute --------
makefile_state/prepare_diffs_to_compute.touch: makefile_state/build_updates.touch
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/prepare_diffs_to_compute.sql
	touch makefile_state/prepare_diffs_to_compute.touch

.PHONY: prepare_diffs_to_compute
prepare_diffs_to_compute: makefile_state/prepare_diffs_to_compute.touch

.PHONY: clean_prepare_diffs_to_compute
clean_prepare_diffs_to_compute: 
	if [ -f makefile_state/prepare_diffs_to_compute.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/prepare_diffs_to_compute.sql; else true; fi
	rm -f makefile_state/prepare_diffs_to_compute.touch

# -------- security_replaced_versions --------
makefile_state/security_replaced_versions.touch: makefile_state/setup_analysis.touch
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/security_replaced_versions.sql
	touch makefile_state/security_replaced_versions.touch

.PHONY: security_replaced_versions
security_replaced_versions: makefile_state/security_replaced_versions.touch

.PHONY: clean_security_replaced_versions
clean_security_replaced_versions: 
	if [ -f makefile_state/security_replaced_versions.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/security_replaced_versions.sql; else true; fi
	rm -f makefile_state/security_replaced_versions.touch

# -------- setup_analysis --------
makefile_state/setup_analysis.touch: 
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/setup_analysis.sql
	touch makefile_state/setup_analysis.touch

.PHONY: setup_analysis
setup_analysis: makefile_state/setup_analysis.touch

.PHONY: clean_setup_analysis
clean_setup_analysis: clean_constraint_types clean_possible_direct_dev_deps clean_possible_direct_runtime_deps clean_possible_version_direct_runtime_deps clean_security_replaced_versions clean_version_ordering_validation clean_vulnerable_versions
	if [ -f makefile_state/setup_analysis.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/setup_analysis.sql; else true; fi
	rm -f makefile_state/setup_analysis.touch

# -------- subsampled_possible_install_deps --------
makefile_state/subsampled_possible_install_deps.touch: makefile_state/possible_install_deps.touch
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/subsampled_possible_install_deps.sql
	touch makefile_state/subsampled_possible_install_deps.touch

.PHONY: subsampled_possible_install_deps
subsampled_possible_install_deps: makefile_state/subsampled_possible_install_deps.touch

.PHONY: clean_subsampled_possible_install_deps
clean_subsampled_possible_install_deps: 
	if [ -f makefile_state/subsampled_possible_install_deps.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/subsampled_possible_install_deps.sql; else true; fi
	rm -f makefile_state/subsampled_possible_install_deps.touch

# -------- subsampled_updates --------
makefile_state/subsampled_updates.touch: makefile_state/build_updates.touch
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/subsampled_updates.sql
	touch makefile_state/subsampled_updates.touch

.PHONY: subsampled_updates
subsampled_updates: makefile_state/subsampled_updates.touch

.PHONY: clean_subsampled_updates
clean_subsampled_updates: 
	if [ -f makefile_state/subsampled_updates.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/subsampled_updates.sql; else true; fi
	rm -f makefile_state/subsampled_updates.touch

# -------- unique_package_deps --------
makefile_state/unique_package_deps.touch: makefile_state/constraint_types.touch
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/unique_package_deps.sql
	touch makefile_state/unique_package_deps.touch

.PHONY: unique_package_deps
unique_package_deps: makefile_state/unique_package_deps.touch

.PHONY: clean_unique_package_deps
clean_unique_package_deps: 
	if [ -f makefile_state/unique_package_deps.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/unique_package_deps.sql; else true; fi
	rm -f makefile_state/unique_package_deps.touch

# -------- version_ordering_validation --------
makefile_state/version_ordering_validation.touch: makefile_state/setup_analysis.touch
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/version_ordering_validation.sql
	touch makefile_state/version_ordering_validation.touch

.PHONY: version_ordering_validation
version_ordering_validation: makefile_state/version_ordering_validation.touch

.PHONY: clean_version_ordering_validation
clean_version_ordering_validation: clean_build_updates
	if [ -f makefile_state/version_ordering_validation.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/version_ordering_validation.sql; else true; fi
	rm -f makefile_state/version_ordering_validation.touch

# -------- vuln_intro_updates --------
makefile_state/vuln_intro_updates.touch: makefile_state/build_updates.touch makefile_state/vulnerable_versions.touch
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/vuln_intro_updates.sql
	touch makefile_state/vuln_intro_updates.touch

.PHONY: vuln_intro_updates
vuln_intro_updates: makefile_state/vuln_intro_updates.touch

.PHONY: clean_vuln_intro_updates
clean_vuln_intro_updates: 
	if [ -f makefile_state/vuln_intro_updates.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/vuln_intro_updates.sql; else true; fi
	rm -f makefile_state/vuln_intro_updates.touch

# -------- vulnerable_versions --------
makefile_state/vulnerable_versions.touch: makefile_state/setup_analysis.touch
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/vulnerable_versions.sql
	touch makefile_state/vulnerable_versions.touch

.PHONY: vulnerable_versions
vulnerable_versions: makefile_state/vulnerable_versions.touch

.PHONY: clean_vulnerable_versions
clean_vulnerable_versions: clean_vuln_intro_updates
	if [ -f makefile_state/vulnerable_versions.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/vulnerable_versions.sql; else true; fi
	rm -f makefile_state/vulnerable_versions.touch

