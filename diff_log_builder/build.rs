use std::env;
use std::fs::read_dir;
use std::fs::DirEntry;
use std::fs::File;
use std::io::Write;
use std::path::Path;

// Adapated from: https://blog.cyplo.dev/posts/2018/12/generate-rust-tests-from-data/


// build script's entry point
fn main() {
    generate_deserialize_seq_tests();
}

fn generate_deserialize_seq_tests() {
    println!("cargo:rerun-if-changed=\"resources/\"");

    let out_dir = env::var("OUT_DIR").unwrap();
    let destination = Path::new(&out_dir).join("deserialize_seq_tests.rs");
    let mut test_file = File::create(&destination).unwrap();

    // write test file header, put `use`, `const` etc there
    write_header(&mut test_file);

    let test_changes_files = read_dir("./resources/test_changes/input/").unwrap();

    for file in test_changes_files {
        write_test(&mut test_file, &file.unwrap());
    }
}

fn write_test(test_file: &mut File, input_file: &DirEntry) {

    if !input_file.file_type().unwrap().is_file() {
        return
    }

    let input_path = input_file.path().canonicalize().unwrap();

    if input_path.extension().unwrap_or_default().to_str().unwrap() != "json" {
        return
    }


    let mut path_correct = Path::new("./resources/test_changes/correct/").to_path_buf().canonicalize().unwrap();
    path_correct.push(input_path.file_name().unwrap());
    path_correct.set_extension("ron");

    let input_name = input_path.file_stem().unwrap().to_string_lossy();

    if path_correct.exists() {
        let test_name = format!("check_correct_deserialize_{}", input_name);

        assert!(path_correct.is_file());

        write!(
            test_file,
            include_str!("./tests/deserialize_seq_test.rs.template"),
            name = test_name,
            path_input = input_path.display(),
            path_correct = path_correct.display()
        ).unwrap();
    } else {
        let test_name = format!("generate_correct_deserialize_{}", input_name);

        write!(
            test_file,
            include_str!("./tests/deserialize_seq_test_make_correct.rs.template"),
            name = test_name,
            path_input = input_path.display(),
            path_correct = path_correct.display()
        ).unwrap();
    }
}

fn write_header(test_file: &mut File) {
    write!(
        test_file,
        r#"
use postgres_db::change_log::Change;
use diff_log_builder::deserialize_change;
"#
    )
    .unwrap();
}