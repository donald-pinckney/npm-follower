use std::process::Command;

// build script's entry point
fn main() {
    npm_install();
}

fn npm_install() {    

    println!("cargo:rerun-if-changed=\"js_parser/\"");

    let output = Command::new("npm")
                                 .current_dir("js_parser")
                                 .arg("install")
                                 .output()
                                 .expect("failed to execute npm subprocess");
    if !output.status.success() {
        panic!("npm failed. stdout:\n{}\n\nstderr:\n{}", String::from_utf8(output.stdout).unwrap(), String::from_utf8(output.stderr).unwrap());
    }
}
