use std::io::Write;
use std::{collections::BTreeMap, fs::File};

const PSQL_COMMAND: &str = "psql -d npm_data -v ON_ERROR_STOP=1 -a -f";

fn main() -> Result<(), std::io::Error> {
    #[rustfmt::skip]
    let dependencies: BTreeMap<&'static str, Vec<&'static str>> = [
        ("setup_analysis", vec![]),
        ("version_ordering_validation", vec!["setup_analysis"]),
        ("build_updates", vec!["version_ordering_validation"]),
        ("find_patches", vec!["build_updates"]),
        ("vulnerable_versions", vec!["setup_analysis"]),
        ("prepare_diffs_to_compute", vec!["build_updates"]),
        ("possible_direct_dev_deps", vec!["setup_analysis"]),
        ("possible_direct_runtime_deps", vec!["setup_analysis"]),
        ("possible_transitive_runtime_deps", vec!["possible_direct_runtime_deps"]),
        ("possible_install_deps", vec!["possible_direct_dev_deps", "possible_direct_runtime_deps", "possible_transitive_runtime_deps"]),
        ("deps_stats", vec!["possible_direct_dev_deps", "possible_direct_runtime_deps", "possible_transitive_runtime_deps", "possible_install_deps"]),
        ("subsampled_possible_install_deps", vec!["possible_install_deps"]),
        ("subsampled_updates", vec!["build_updates"]),

    ]
    .into_iter()
    .collect();

    // Check that dependencies is closed
    for deps in dependencies.values() {
        for d in deps {
            assert!(dependencies.contains_key(d))
        }
    }

    let mut reverse_dependencies: BTreeMap<&'static str, Vec<&'static str>> = dependencies
        .iter()
        .flat_map(|(step, depends_on)| depends_on.iter().map(move |dep| (dep, step)))
        .fold(BTreeMap::new(), |mut acc, (k, v)| {
            acc.entry(k).or_insert_with(Vec::new).push(v);
            acc
        });

    for node in dependencies.keys() {
        if !reverse_dependencies.contains_key(node) {
            reverse_dependencies.insert(node, vec![]);
        }
    }

    let mut output_file = File::create("Makefile")?;

    writeln!(output_file, ".PHONY: all")?;
    let all_nodes: Vec<_> = dependencies.keys().cloned().collect();
    writeln!(output_file, "all: {}", all_nodes.join(" "))?;
    writeln!(output_file)?;

    // writeln!(output_file, ".PHONY: clean")?;
    // let all_clean_nodes: Vec<_> = dependencies
    //     .keys()
    //     .cloned()
    //     .map(|n| format!("clean_{}", n))
    //     .collect();
    // writeln!(output_file, "clean: {}", all_clean_nodes.join(" "))?;
    // writeln!(output_file)?;

    for (step, depends_on) in dependencies {
        let mut depends_on_sorted = depends_on.clone();
        depends_on_sorted.sort();

        let deps = depends_on_sorted
            .iter()
            .map(|d| format!("makefile_state/{}.touch", *d))
            .collect::<Vec<_>>()
            .join(" ");

        writeln!(output_file, "# -------- {} --------", step)?;
        writeln!(output_file, "makefile_state/{}.touch: {}", step, deps)?;
        writeln!(output_file, "\t{} scripts/{}.sql", PSQL_COMMAND, step)?;
        writeln!(output_file, "\ttouch makefile_state/{}.touch", step)?;
        writeln!(output_file)?;
        writeln!(output_file, ".PHONY: {}", step)?;
        writeln!(output_file, "{}: makefile_state/{}.touch", step, step)?;

        writeln!(output_file)?;
        writeln!(output_file, ".PHONY: clean_{}", step)?;
        let mut rev_deps_sorted: Vec<_> = reverse_dependencies[step]
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
