extern crate bindgen;
use std::path::Path;
use std::env;
use std::{
    process::Command,
};
use bindgen::builder;


fn main() {

    Command::new("make")
        .arg("clean")
        .arg("libstarcode.a")
        .status()
        .expect("failed to make!");

    println!("cargo:rustc-link-lib=starcode");
    //println!("cargo:rustc-link-search={}", out_path.display());
    let dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo:rustc-link-search=native={}", Path::new(&dir).display());

    // Configure and generate bindings.
    let bindings = builder().header("wrapper.h")
        .generate().expect("Unable to create bindings");

    // Write the generated bindings to an output file.
    bindings.write_to_file("bindings.rs").expect("Unable to write the output binding file");
}