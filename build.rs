extern crate bindgen;

use std::{
    env,
    path::PathBuf,
    process::Command,
};
use bindgen::builder;


fn main() {

    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    let output = Command::new("make")
        .arg("clean")
        .arg("libstarcode.a")
        .status()
        .expect("failed to make!");

    println!("cargo:rustc-link-lib=starcode");
    //println!("cargo:rustc-link-search={}", out_path.display());
    println!("cargo:rustc-link-search=./");


    // Configure and generate bindings.
    let bindings = builder().header("wrapper.h")
        .generate().expect("Unable to create bindings");

    // Write the generated bindings to an output file.
    bindings.write_to_file("bindings.rs").expect("Unable to write the output binding file");
}