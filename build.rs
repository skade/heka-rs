// build.rs
// Right now this is more or less a literal translation of build.sh.
// Something needs to be done about protobuf code generation (which itself has
// a rust dependency).

use std::process::Command;
use std::process::exit;

use std::path::Path;
// original script is *NIX-only
use std::path::MAIN_SEPARATOR;

fn main() {
    let out_dir = std::env::var_os("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir);
    let root_path = std::env::current_dir().unwrap();

    std::env::set_current_dir(&out_path).unwrap();

    let cmake_status = match Command::new("cmake").arg(
        "-DCMAKE_BUILD_TYPE=release").arg(root_path).status() {
        Ok(status) => status,
        Err(e) => panic!("failed to execute process: {}", e),
    };

    let cmake_ret = cmake_status.code().unwrap();

    if cmake_ret != 0 {
        exit(cmake_ret);
    }

    let make_status = match Command::new("make").status() {
        Ok(status) => status,
        Err(e) => panic!("failed to execute process: {}", e),
    };

    let make_ret = make_status.code().unwrap();

    if make_ret != 0 {
        exit(make_ret);
    }

    println!("cargo:rustc-flags=-L {:?}{:?}lib", &out_dir, MAIN_SEPARATOR);
}
