use std::collections::HashMap;
use std::env;
use std::fs::read_dir;
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

// Adapated from: https://blog.cyplo.dev/posts/2018/12/generate-rust-tests-from-data/

static TEST_BUILDER_CONFIGS: &[InputOutputTestBuilderConfig] = &[
    InputOutputTestBuilderConfig {
        test_suite_name: "deserialize_change",
        header: r#"
        "#,
    },
    InputOutputTestBuilderConfig {
        test_suite_name: "process_changes",
        header: r#"
            "#,
    },
];

struct InputOutputTestBuilderConfig {
    test_suite_name: &'static str,
    header: &'static str,
}

fn format_io_err<T, P>(r: io::Result<T>, path: P) -> Result<T, String>
where
    P: AsRef<Path>,
{
    r.map_err(|err| {
        format!(
            "Error with path {}:\n{}",
            path.as_ref().to_str().unwrap(),
            err
        )
    })
}

impl InputOutputTestBuilderConfig {
    fn test_resources_path(&self) -> PathBuf {
        ["resources", &format!("test_{}", self.test_suite_name)]
            .iter()
            .collect()
    }

    fn read_all_files_from_dir(
        &self,
        dir: PathBuf,
        extension: &str,
    ) -> Result<HashMap<String, PathBuf>, String> {
        let mut input_files = HashMap::new();
        dbg!(&dir);
        for f in format_io_err(read_dir(&dir), dir)? {
            let f = f.unwrap();
            if !f.file_type().unwrap().is_file() {
                continue;
            }

            let f_path = f.path().canonicalize().unwrap();

            if f_path.extension().unwrap_or_default().to_str().unwrap() != extension {
                continue;
            }

            let f_name = f_path.file_stem().unwrap().to_str().unwrap().to_string();

            input_files.insert(f_name, f_path);
        }

        Ok(input_files)
    }

    fn input_files(&self) -> Result<HashMap<String, PathBuf>, String> {
        let mut input_path = self.test_resources_path();
        input_path.push("input");
        self.read_all_files_from_dir(input_path, "json")
    }

    fn correct_files(&self) -> Result<HashMap<String, PathBuf>, String> {
        let mut input_path = self.test_resources_path();
        input_path.push("correct");
        self.read_all_files_from_dir(input_path, "ron")
    }

    fn dst_path(&self) -> PathBuf {
        [
            env::var("OUT_DIR").unwrap(),
            format!("{}_tests.rs", self.test_suite_name),
        ]
        .iter()
        .collect()
    }

    fn template_check_path(&self) -> PathBuf {
        [
            "tests",
            &format!("test_{}.rs.template", self.test_suite_name),
        ]
        .iter()
        .collect()
    }

    fn template_make_path(&self) -> PathBuf {
        [
            "tests",
            &format!("test_{}_make_correct.rs.template", self.test_suite_name),
        ]
        .iter()
        .collect()
    }

    fn generate(&self) -> Result<(), String> {
        let input_files = self.input_files()?;
        let correct_files = self.correct_files()?;

        let mut test_file = File::create(self.dst_path()).unwrap();
        writeln!(test_file, "{}", self.header).unwrap();

        for f in correct_files.iter() {
            if !input_files.contains_key(f.0) {
                println!(
                    "cargo:warning=\"{} has no corresponding input file.\"",
                    f.1.to_str().unwrap()
                );
            }
        }

        for (name, input_path) in input_files {
            if let Some(correct_path) = correct_files.get(&name) {
                self.generate_check_correct(&mut test_file, name, &input_path, correct_path)?
            } else {
                let mut new_correct_path = self.test_resources_path();
                new_correct_path.push("correct");
                new_correct_path.push(format!("{}.ron", name));
                self.generate_make_correct(&mut test_file, name, &input_path, &new_correct_path)?
            }
        }

        Ok(())
    }

    fn generate_check_correct(
        &self,
        test_file: &mut File,
        name: String,
        input_path: &Path,
        correct_path: &Path,
    ) -> Result<(), String> {
        let input_path = input_path.canonicalize().unwrap();

        let test_name = format!("test_{}_{}_check", self.test_suite_name, name);

        assert!(correct_path.is_file());

        let template_path = self.template_check_path();
        let test_template = format_io_err(std::fs::read_to_string(&template_path), template_path)?;

        let gen_test = test_template
            .replace("$TEST_NAME", &test_name)
            .replace("$INPUT_PATH", input_path.to_str().unwrap())
            .replace("$CORRECT_PATH", correct_path.to_str().unwrap());

        writeln!(test_file, "{}", gen_test).unwrap();

        Ok(())
    }

    fn generate_make_correct(
        &self,
        test_file: &mut File,
        name: String,
        input_path: &Path,
        correct_path: &Path,
    ) -> Result<(), String> {
        let input_path = input_path.canonicalize().unwrap();

        let test_name = format!("test_{}_{}_make", self.test_suite_name, name);

        let template_path = self.template_make_path();
        let test_template = format_io_err(std::fs::read_to_string(&template_path), template_path)?;

        let gen_test = test_template
            .replace("$TEST_NAME", &test_name)
            .replace("$INPUT_PATH", input_path.to_str().unwrap())
            .replace("$CORRECT_PATH", correct_path.to_str().unwrap());

        writeln!(test_file, "{}", gen_test).unwrap();

        Ok(())
    }
}

// build script's entry point
fn main() {
    println!("cargo:rerun-if-changed=\"build.rs\"");
    println!("cargo:rerun-if-changed=\"resources/\"");

    for c in TEST_BUILDER_CONFIGS {
        if let Err(err) = c.generate() {
            let error_message = format!(
                "Error generating I/O test suite {}:\n{}",
                c.test_suite_name, err
            );
            error_message
                .lines()
                .for_each(|l| println!("cargo:warning=\"{}\"", l));
            panic!("{}", error_message);
        }
    }
}
