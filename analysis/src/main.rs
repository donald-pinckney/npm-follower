use std::io::Write;
use std::{collections::BTreeMap, fs::File};

const PSQL_COMMAND: &str = "psql -d npm_data -v ON_ERROR_STOP=1 -a -f";

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum StepType {
    Sql,
    Rust,
}

impl StepType {
    fn generate_shell_cmd(&self, step_name: &'static str) -> String {
        match self {
            StepType::Sql => format!("{} sql/{}.sql", PSQL_COMMAND, step_name),
            StepType::Rust => format!("cd rust; cargo run --release --bin {}", step_name),
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct AnalysisStep {
    name: &'static str,
    step_type: StepType,
}

trait SqlOrRust {
    fn sql(&self) -> AnalysisStep;
    fn rust(&self) -> AnalysisStep;
}

impl SqlOrRust for &'static str {
    fn sql(&self) -> AnalysisStep {
        AnalysisStep {
            name: self,
            step_type: StepType::Sql,
        }
    }

    fn rust(&self) -> AnalysisStep {
        AnalysisStep {
            name: self,
            step_type: StepType::Rust,
        }
    }
}

struct AnalysisDag {
    steps: BTreeMap<&'static str, (StepType, Vec<&'static str>)>,
}

impl AnalysisDag {
    fn new() -> Self {
        AnalysisDag {
            steps: BTreeMap::new(),
        }
    }

    fn add_step(&mut self, step: AnalysisStep, depends_on: Vec<&'static str>) {
        self.steps.insert(step.name, (step.step_type, depends_on));
    }

    fn get_step_type(&self, step: &'static str) -> StepType {
        self.steps[step].0
    }

    fn get_dependencies(&self, step: &'static str) -> Vec<&'static str> {
        self.steps[step].1.clone()
    }

    fn get_all_step_names(&self) -> Vec<&'static str> {
        self.steps.keys().cloned().collect()
    }

    fn get_reverse_dependencies(&self, step: &'static str) -> Vec<&'static str> {
        self.steps
            .iter()
            .filter(|(_, deps)| deps.1.contains(&step))
            .map(|(step, _)| *step)
            .collect()
    }

    fn assert_closure(&self) {
        for (step, depends_on) in self.steps.iter() {
            for d in &depends_on.1 {
                assert!(
                    self.steps.contains_key(d),
                    "Step {} depends on {}, but {} is not a step",
                    step,
                    d,
                    d
                );
            }
        }
    }
}

fn main() -> Result<(), std::io::Error> {
    let mut dependencies: AnalysisDag = AnalysisDag::new();

    #[rustfmt::skip]
    {
        dependencies.add_step("setup_analysis".sql(), vec![]);
        dependencies.add_step("constraint_types".sql(), vec!["setup_analysis"]);
        dependencies.add_step("unique_package_deps".sql(), vec!["constraint_types"]);
        dependencies.add_step("version_ordering_validation".sql(), vec!["setup_analysis"]);
        dependencies.add_step("build_updates".sql(), vec!["version_ordering_validation"]);
        dependencies.add_step("find_patches".sql(), vec!["build_updates"]);
        dependencies.add_step("vulnerable_versions".sql(), vec!["setup_analysis"]);
        dependencies.add_step("vuln_intro_updates".sql(), vec!["vulnerable_versions", "build_updates"]);
        dependencies.add_step("prepare_diffs_to_compute".sql(), vec!["build_updates"]);
        dependencies.add_step("possible_direct_dev_deps".sql(), vec!["setup_analysis"]);
        dependencies.add_step("possible_direct_runtime_deps".sql(), vec!["setup_analysis"]);
        dependencies.add_step("possible_version_direct_runtime_deps".sql(), vec!["setup_analysis"]);
        dependencies.add_step("possible_transitive_runtime_deps".sql(), vec!["possible_direct_runtime_deps"]);
        dependencies.add_step("possible_install_deps".sql(), vec!["possible_direct_dev_deps", "possible_direct_runtime_deps", "possible_transitive_runtime_deps"]);
        dependencies.add_step("deps_stats".sql(), vec!["possible_direct_dev_deps", "possible_direct_runtime_deps", "possible_transitive_runtime_deps", "possible_install_deps"]);
        dependencies.add_step("subsampled_possible_install_deps".sql(), vec!["possible_install_deps"]);
        dependencies.add_step("subsampled_updates".sql(), vec!["build_updates"]);
        dependencies.add_step("security_replaced_versions".sql(), vec!["setup_analysis"]);
        dependencies.add_step("possibly_malicious_packages".rust(), vec!["security_replaced_versions"]);
        dependencies.add_step("unpublished_versions".rust(), vec!["setup_analysis"]);
    };

    // Check that dependencies is closed
    dependencies.assert_closure();

    let mut output_file = File::create("Makefile")?;

    writeln!(output_file, ".PHONY: all")?;
    let all_nodes: Vec<_> = dependencies.get_all_step_names();
    writeln!(output_file, "all: {}", all_nodes.join(" "))?;
    writeln!(output_file)?;

    for step in all_nodes {
        let depends_on = dependencies.get_dependencies(step);
        let mut depends_on_sorted = depends_on.clone();
        depends_on_sorted.sort();

        let deps = depends_on_sorted
            .iter()
            .map(|d| format!("makefile_state/{}.touch", *d))
            .collect::<Vec<_>>()
            .join(" ");

        writeln!(output_file, "# -------- {} --------", step)?;
        writeln!(output_file, "makefile_state/{}.touch: {}", step, deps)?;
        writeln!(
            output_file,
            "\t{}",
            dependencies.get_step_type(step).generate_shell_cmd(step)
        )?;
        writeln!(output_file, "\ttouch makefile_state/{}.touch", step)?;
        writeln!(output_file)?;
        writeln!(output_file, ".PHONY: {}", step)?;
        writeln!(output_file, "{}: makefile_state/{}.touch", step, step)?;

        writeln!(output_file)?;
        writeln!(output_file, ".PHONY: clean_{}", step)?;
        let mut rev_deps_sorted: Vec<_> = dependencies
            .get_reverse_dependencies(step)
            .iter()
            .map(|rd| format!("clean_{}", rd))
            .collect();
        rev_deps_sorted.sort();
        writeln!(output_file, "clean_{}: {}", step, rev_deps_sorted.join(" "))?;
        writeln!(
            output_file,
            "\tif [ -f makefile_state/{}.touch ]; then {} clean/{}.sql; else true; fi",
            step, PSQL_COMMAND, step
        )?;
        writeln!(output_file, "\trm -f makefile_state/{}.touch", step)?;
        writeln!(output_file)?;
    }

    Ok(())
}
