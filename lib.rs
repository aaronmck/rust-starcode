
extern crate libc;
extern crate tempfile;
extern crate bindgen;
extern crate rand;

use std::convert::TryFrom;
use std::ffi::CString;
use libc::{c_char, c_int, c_uint, c_void};
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, Write};
use std::path::Path;
use tempfile::NamedTempFile;

#[allow(non_upper_case_globals)]
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
include!("bindings.rs");

pub struct StarcodeAlignment {
    cluster_centers: Vec<String>,
    cluster_count: Vec<usize>,
    cluster_members: Vec<Vec<String>>,
}

fn write_vectors_to_file(filename: &Path, vectors: &Vec<Vec<u8>>) -> io::Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)  // Use this if you want to append to an existing file
        .open(filename)?;

    for data in vectors {
        file.write_all(&data)?;
        file.write_all(b"\n")?; // Optional: Adds a newline between entries
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
    /// # Examples
    ///
    /// ```
    /// let sequences = vec![
    ///     vec![b'A', b'T', b'C', b'G'],
    ///     vec![b'A', b'T', b'C', b'A'],
    ///     vec![b'G', b'T', b'C', b'A']
    /// ];
    /// let max_distance = 2;
    /// let alignment = StarcodeAlignment::align_sequences(&sequences, &max_distance);
    /// ```
    pub fn align_sequences(
        sequences: &Vec<Vec<u8>>,
        max_distance: &i32,
    ) -> StarcodeAlignment {

        assert!(*max_distance >= 0);

        // write out the sequences to a fastq file
        let temp_input_file = NamedTempFile::new().expect("Failed to create temporary input file");
        let temp_output_file = NamedTempFile::new().expect("Failed to create temporary output file");

        // Get the temporary file path
        let temp_input_path = temp_input_file.path();
        let temp_output_path = temp_output_file.path();

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
                            1.0, // default from StarCode codebase
                            1, // show the clusters?
                            0,
                            0
            );
        }

        recover_cluster_entries_from_file(temp_output_path.to_str().unwrap())

    }
}

fn split_line(line: &str) -> (String, String, Option<Vec<String>>) {
    let tokens: Vec<&str> = line.split_whitespace().collect();

    let first = tokens.get(0).unwrap_or(&"").to_string();
    let second = tokens.get(1).unwrap_or(&"").to_string();

    if tokens.len() > 2 {
        let third_token = tokens[2];
        let third_list: Vec<String> = third_token.split(',').map(|s| s.to_string()).collect();
        (first, second, Some(third_list))
    } else {
        (first, second, None)
    }
}



fn recover_cluster_entries_from_file(file_path: &str) -> StarcodeAlignment {

    let mut cluster_centers: Vec<String> = Vec::new();
    let mut cluster_count: Vec<usize> = Vec::new();
    let mut cluster_members: Vec<Vec<String>> = Vec::new();

    // Open the file
    let file = File::open(file_path).expect("Unable to open starcode clustering output");

    // Create a buffered reader
    let reader = io::BufReader::new(file);

    // Iterate over each line in the file
    for line in reader.lines() {
        // Print each line
        let spt = split_line(line.unwrap().as_str());
        assert_ne!("",spt.0);
        assert_ne!("",spt.1);
        cluster_centers.push(spt.0.clone());
        cluster_count.push(spt.1.parse().unwrap());
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

    fn random_10mers() -> Vec<Vec<u8>> {
        let num_sequences = 10000;
        let sequence_length = 10;

        (0..num_sequences)
            .map(|_| generate_random_nucleotide_sequence(sequence_length))
            .map(|(s,c)| format!("{} {}",String::from_utf8(s).unwrap(),c).as_bytes().to_vec()).collect()

    }

    #[test]
    fn test_add() {
        let alignment = StarcodeAlignment::align_sequences(&random_10mers(),&2);
    }
}
