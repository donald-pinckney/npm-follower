use std::env;
use std::fs::read_dir;
use std::fs::DirEntry;
use std::fs::File;
use std::io::Write;
use std::path::Path;

// From: https://blog.cyplo.dev/posts/2018/12/generate-rust-tests-from-data/


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

    let test_changes_files = read_dir("./resources/test_changes/").unwrap();

    for file in test_changes_files {
        write_test(&mut test_file, &file.unwrap());
    }
}

fn write_test(test_file: &mut File, data_file: &DirEntry) {

    if !data_file.file_type().unwrap().is_file() {
        return
    }

    let data_path = data_file.path().canonicalize().unwrap();

    if data_path.extension().unwrap_or_default().to_str().unwrap() != "json" {
        return
    }

    let data_name = data_path.file_stem().unwrap().to_string_lossy();
    let test_name = format!("test_deserialize_{}", data_name);

    write!(
        test_file,
        include_str!("./tests/deserialize_seq_test.rs.template"),
        name = test_name,
        path = data_path.display()
    )
    .unwrap();
}

fn write_header(test_file: &mut File) {
    write!(
        test_file,
        r#"
use postgres_db::change_log::Change;
use relational_db_builder::deserialize_change;
"#
    )
    .unwrap();
}