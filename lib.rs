

#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

// suppress warnings about u128 (that we don't even use ourselves anyway)
#![allow(improper_ctypes)]

extern crate libc;
extern crate tempfile;
extern crate bindgen;
extern crate rand;
extern crate rustc_hash;

use std::ffi::CString;
use libc::{c_char};
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, Write};
use std::path::Path;
use tempfile::NamedTempFile;
use rustc_hash::{FxHashMap};


include!("bindings.rs");

#[allow(dead_code)]
pub struct StarcodeAlignment {
    pub cluster_centers: Vec<Vec<u8>>,
    pub cluster_count: Vec<usize>,
    pub cluster_members: Vec<Vec<Vec<u8>>>,
}

fn write_vectors_to_file(filename: &Path, vectors: &FxHashMap<Vec<u8>,usize>) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)  // Use this if you want to append to an existing file
        .open(filename)?;

    for data in vectors {
        file.write_all(&data.0)?;
        file.write_all(b"\t")?;
        file.write_all(data.1.to_string().as_bytes())?;
        file.write_all(b"\n")?;
    }

    Ok(())
}


impl StarcodeAlignment {
    /// Aligns a set of sequences using the Starcode algorithm and returns the alignment result.
    ///
    /// This function takes a vector of nucleotide sequences and a maximum distance parameter,
    /// writes the sequences to a temporary file, and then uses the Starcode algorithm to align
    /// the sequences. The result is read back from a temporary output file and returned as a
    /// `StarcodeAlignment`.
    ///
    /// # Arguments
    ///
    /// * `sequences` - A reference to a vector of vectors, where each inner vector represents
    /// a sequence of nucleotides as bytes.
    /// * `max_distance` - A reference to an integer specifying the maximum allowed distance for
    /// clustering sequences.
    ///
    /// # Returns
    ///
    /// A `StarcodeAlignment` containing the alignment result.
    ///
    /// # Panics
    ///
    /// This function will panic if the temporary input or output files cannot be created,
    /// if writing to the temporary input file fails, or if the `max_distance` is negative.
    ///
    pub fn align_sequences(
        sequences: &FxHashMap<Vec<u8>,usize>,
        max_distance: &i32,
        parent_to_child_ratio: &f64,
    ) -> StarcodeAlignment {

        assert!(*max_distance >= 0);

        // write out the sequences to a fastq file
        let temp_input_file = NamedTempFile::new().expect("Failed to create temporary input file");
        let temp_output_file = NamedTempFile::new().expect("Failed to create temporary output file");

        // Get the temporary file path
        let temp_input_path = temp_input_file.path();
        let temp_output_path = temp_output_file.path();
        println!("input path {:?} output path {:?}",temp_input_path,temp_output_path);

        write_vectors_to_file(temp_input_path, sequences).expect("Unable to write to temp file when running StarCode");

        unsafe {
            let input_file_path = CString::new(temp_input_file.path().to_str().unwrap()).unwrap();
            let inp: *mut c_char = input_file_path.into_raw(); //std::ptr::null_mut(); foo(&mut s);

            let output_file_path = CString::new(temp_output_path.to_str().unwrap()).unwrap();
            let outp: *mut c_char = output_file_path.into_raw(); //std::ptr::null_mut(); foo(&mut s);

            starcode_helper(inp,
                            outp,
                            *max_distance,
                            0, // no stdout
                            1, // one thread
                            0, // message passing
                            *parent_to_child_ratio, // default from StarCode codebase
                            1, // show the clusters?
                            0,
                            0
            );
        }

        recover_cluster_entries_from_file(temp_output_path.to_str().unwrap())

    }
}

fn split_line(line: &str) -> (Vec<u8>, Vec<u8>, Option<Vec<Vec<u8>>>) {
    let tokens: Vec<&str> = line.split_whitespace().collect();

    let first = tokens.get(0).unwrap_or(&"").as_bytes().to_vec();
    let second = tokens.get(1).unwrap_or(&"").as_bytes().to_vec();

    if tokens.len() > 2 {
        let third_token = tokens[2];
        let third_list: Vec<Vec<u8>> = third_token.split(',').map(|s| s.as_bytes().to_vec()).collect();
        (first, second, Some(third_list))
    } else {
        (first, second, None)
    }
}



fn recover_cluster_entries_from_file(file_path: &str) -> StarcodeAlignment {

    let mut cluster_centers: Vec<Vec<u8>> = Vec::new();
    let mut cluster_count: Vec<usize> = Vec::new();
    let mut cluster_members: Vec<Vec<Vec<u8>>> = Vec::new();

    // Open the file
    let file = File::open(file_path).expect("Unable to open starcode clustering output");

    // Create a buffered reader
    let reader = io::BufReader::new(file);

    // Iterate over each line in the file
    for line in reader.lines() {
        // Print each line
        let spt = split_line(line.unwrap().as_str());
        assert_ne!(spt.0.len(),0);
        assert_ne!(spt.0.len(),0);
        cluster_centers.push(spt.0.clone());
        cluster_count.push(String::from_utf8(spt.1).unwrap().parse().unwrap());
        match spt.2 {
            None => {
                cluster_members.push(Vec::new());
            }
            Some(x) => {
                cluster_members.push(x);
            }
        }

    }

    StarcodeAlignment{
        cluster_centers,
        cluster_count,
        cluster_members,
    }
}
#[allow(dead_code)]
fn print_lines_from_file(file_path: &str) -> io::Result<()> {
    // Open the file
    let file = File::open(file_path)?;

    // Create a buffered reader
    let reader = io::BufReader::new(file);

    // Iterate over each line in the file
    for line in reader.lines() {
        // Print each line
        println!("{}", line?);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::prelude::IndexedRandom;

    fn generate_random_nucleotide_sequence(length: usize) -> (Vec<u8>,usize) {
        let nucleotides = b"ATCG";
        let mut rng = rand::thread_rng();
        ((0..length)
            .map(|_| *nucleotides.choose(&mut rng).unwrap())
            .collect(),1)
    }

    fn random_10mers() -> FxHashMap<Vec<u8>,usize> {
        let num_sequences = 10000;
        let sequence_length = 10;
        let mut ret: FxHashMap<Vec<u8>,usize> = FxHashMap::default();
        for _i in 0..num_sequences {
            let k_v = generate_random_nucleotide_sequence(sequence_length);
            ret.insert(k_v.0,k_v.1);
        }
        ret
    }

    #[test]
    fn test_basic_add() {
        let _alignment = StarcodeAlignment::align_sequences(&random_10mers(),&2, &2.0);
    }

    #[test]
    fn test_known_merge_situation() {
        let mut knowns: FxHashMap<Vec<u8>,usize> = FxHashMap::default();
        knowns.insert("AAAAAAAAAA".as_bytes().to_vec(),1);
        knowns.insert("CCCCCCCCCC".as_bytes().to_vec(),1);
        knowns.insert("GGGGGGGGGG".as_bytes().to_vec(),1);
        knowns.insert("TTTTTTTTTT".as_bytes().to_vec(),1);
        let alignment = StarcodeAlignment::align_sequences(&knowns,&2, &2.0);

        assert_eq!(alignment.cluster_centers.len(),4);

        let mut knowns: FxHashMap<Vec<u8>,usize> = FxHashMap::default();
        knowns.insert("AAAAAAAAAA".as_bytes().to_vec(),1);
        knowns.insert("AAAAAAAAAC".as_bytes().to_vec(),1);
        knowns.insert("GGGGGGGGGG".as_bytes().to_vec(),1);
        knowns.insert("TTTTTTTTTT".as_bytes().to_vec(),1);
        let alignment = StarcodeAlignment::align_sequences(&knowns,&2, &1.0);

        assert_eq!(alignment.cluster_centers.len(),3);

        let alignment = StarcodeAlignment::align_sequences(&knowns,&2, &2.0);

        assert_eq!(alignment.cluster_centers.len(),4);

    }
}
