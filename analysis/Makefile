.PHONY: all
all: build_updates setup_analysis version_ordering_validation

.PHONY: clean
clean: clean_build_updates clean_setup_analysis clean_version_ordering_validation

# -------- build_updates --------
makefile_state/build_updates.touch:
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/build_updates.sql
	touch makefile_state/build_updates.touch

.PHONY: build_updates
build_updates: version_ordering_validation makefile_state/build_updates.touch

.PHONY: clean_build_updates
clean_build_updates: 
	if [ -f makefile_state/build_updates.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/build_updates.sql; else true; fi
	rm -f makefile_state/build_updates.touch

# -------- setup_analysis --------
makefile_state/setup_analysis.touch:
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/setup_analysis.sql
	touch makefile_state/setup_analysis.touch

.PHONY: setup_analysis
setup_analysis:  makefile_state/setup_analysis.touch

.PHONY: clean_setup_analysis
clean_setup_analysis: clean_version_ordering_validation
	if [ -f makefile_state/setup_analysis.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/setup_analysis.sql; else true; fi
	rm -f makefile_state/setup_analysis.touch

# -------- version_ordering_validation --------
makefile_state/version_ordering_validation.touch:
	psql -d npm_data -v ON_ERROR_STOP=1 -a -f scripts/version_ordering_validation.sql
	touch makefile_state/version_ordering_validation.touch

.PHONY: version_ordering_validation
version_ordering_validation: setup_analysis makefile_state/version_ordering_validation.touch

.PHONY: clean_version_ordering_validation
clean_version_ordering_validation: clean_build_updates
	if [ -f makefile_state/version_ordering_validation.touch ]; then psql -d npm_data -v ON_ERROR_STOP=1 -a -f clean/version_ordering_validation.sql; else true; fi
	rm -f makefile_state/version_ordering_validation.touch

