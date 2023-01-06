.PHONY: all
all: build_updates find_patches possible_direct_dev_version_deps possible_direct_runtime_deps possible_install_deps possible_transitive_runtime_deps prepare_diffs_to_compute setup_analysis version_ordering_validation

.PHONY: clean
clean: clean_build_updates clean_find_patches clean_possible_direct_dev_version_deps clean_possible_direct_runtime_deps clean_possible_install_deps clean_possible_transitive_runtime_deps clean_prepare_diffs_to_compute clean_setup_analysis clean_version_ordering_validation

# -------- build_updates --------
makefile_state/build_updates.touch: makefile_state/version_ordering_validation.touch
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/build_updates.sql
	touch makefile_state/build_updates.touch

.PHONY: build_updates
build_updates: makefile_state/build_updates.touch

.PHONY: clean_build_updates
clean_build_updates: clean_find_patches clean_prepare_diffs_to_compute
	if [ -f makefile_state/build_updates.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/build_updates.sql; else true; fi
	rm -f makefile_state/build_updates.touch

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

# -------- possible_direct_dev_version_deps --------
makefile_state/possible_direct_dev_version_deps.touch: makefile_state/setup_analysis.touch
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/possible_direct_dev_version_deps.sql
	touch makefile_state/possible_direct_dev_version_deps.touch

.PHONY: possible_direct_dev_version_deps
possible_direct_dev_version_deps: makefile_state/possible_direct_dev_version_deps.touch

.PHONY: clean_possible_direct_dev_version_deps
clean_possible_direct_dev_version_deps: clean_possible_install_deps clean_possible_transitive_runtime_deps
	if [ -f makefile_state/possible_direct_dev_version_deps.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/possible_direct_dev_version_deps.sql; else true; fi
	rm -f makefile_state/possible_direct_dev_version_deps.touch

# -------- possible_direct_runtime_deps --------
makefile_state/possible_direct_runtime_deps.touch: makefile_state/setup_analysis.touch
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/possible_direct_runtime_deps.sql
	touch makefile_state/possible_direct_runtime_deps.touch

.PHONY: possible_direct_runtime_deps
possible_direct_runtime_deps: makefile_state/possible_direct_runtime_deps.touch

.PHONY: clean_possible_direct_runtime_deps
clean_possible_direct_runtime_deps: clean_possible_transitive_runtime_deps
	if [ -f makefile_state/possible_direct_runtime_deps.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/possible_direct_runtime_deps.sql; else true; fi
	rm -f makefile_state/possible_direct_runtime_deps.touch

# -------- possible_install_deps --------
makefile_state/possible_install_deps.touch: makefile_state/possible_direct_dev_version_deps.touch makefile_state/possible_transitive_runtime_deps.touch
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/possible_install_deps.sql
	touch makefile_state/possible_install_deps.touch

.PHONY: possible_install_deps
possible_install_deps: makefile_state/possible_install_deps.touch

.PHONY: clean_possible_install_deps
clean_possible_install_deps: 
	if [ -f makefile_state/possible_install_deps.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/possible_install_deps.sql; else true; fi
	rm -f makefile_state/possible_install_deps.touch

# -------- possible_transitive_runtime_deps --------
makefile_state/possible_transitive_runtime_deps.touch: makefile_state/possible_direct_dev_version_deps.touch makefile_state/possible_direct_runtime_deps.touch
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/possible_transitive_runtime_deps.sql
	touch makefile_state/possible_transitive_runtime_deps.touch

.PHONY: possible_transitive_runtime_deps
possible_transitive_runtime_deps: makefile_state/possible_transitive_runtime_deps.touch

.PHONY: clean_possible_transitive_runtime_deps
clean_possible_transitive_runtime_deps: clean_possible_install_deps
	if [ -f makefile_state/possible_transitive_runtime_deps.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/possible_transitive_runtime_deps.sql; else true; fi
	rm -f makefile_state/possible_transitive_runtime_deps.touch

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

# -------- setup_analysis --------
makefile_state/setup_analysis.touch: 
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/setup_analysis.sql
	touch makefile_state/setup_analysis.touch

.PHONY: setup_analysis
setup_analysis: makefile_state/setup_analysis.touch

.PHONY: clean_setup_analysis
clean_setup_analysis: clean_possible_direct_dev_version_deps clean_possible_direct_runtime_deps clean_version_ordering_validation
	if [ -f makefile_state/setup_analysis.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/setup_analysis.sql; else true; fi
	rm -f makefile_state/setup_analysis.touch

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

