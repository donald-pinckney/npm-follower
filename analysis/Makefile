.PHONY: all
all: build_updates setup_analysis version_ordering_validation

.PHONY: clean
clean: clean_build_updates clean_setup_analysis clean_version_ordering_validation

# -------- build_updates --------
makefile_state/build_updates.touch:
	psql -p 5431 -d npm_data -a -f scripts/build_updates.sql
	touch makefile_state/build_updates.touch

.PHONY: build_updates
build_updates: version_ordering_validation makefile_state/build_updates.touch

.PHONY: clean_build_updates
clean_build_updates: 
	psql -p 5431 -d npm_data -a -f clean/build_updates.sql
	rm -f makefile_state/build_updates.touch

# -------- setup_analysis --------
makefile_state/setup_analysis.touch:
	psql -p 5431 -d npm_data -a -f scripts/setup_analysis.sql
	touch makefile_state/setup_analysis.touch

.PHONY: setup_analysis
setup_analysis:  makefile_state/setup_analysis.touch

.PHONY: clean_setup_analysis
clean_setup_analysis: clean_version_ordering_validation
	psql -p 5431 -d npm_data -a -f clean/setup_analysis.sql
	rm -f makefile_state/setup_analysis.touch

# -------- version_ordering_validation --------
makefile_state/version_ordering_validation.touch:
	psql -p 5431 -d npm_data -a -f scripts/version_ordering_validation.sql
	touch makefile_state/version_ordering_validation.touch

.PHONY: version_ordering_validation
version_ordering_validation: setup_analysis makefile_state/version_ordering_validation.touch

.PHONY: clean_version_ordering_validation
clean_version_ordering_validation: clean_build_updates
	psql -p 5431 -d npm_data -a -f clean/version_ordering_validation.sql
	rm -f makefile_state/version_ordering_validation.touch

