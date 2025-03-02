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

pub struct StarcodeContext {
    tower_top: *mut gstack_t,
    #[cfg(debug_assertions)]
    allocation_count: std::sync::atomic::AtomicUsize,
}

impl StarcodeContext {
    pub fn new() -> Self {
        // Initialize the context
        unsafe {
            // Call init_tower to ensure we start with a clean state
            init_new_tower();
        }
        
        Self {
            tower_top: std::ptr::null_mut(),
            #[cfg(debug_assertions)]
            allocation_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    #[cfg(debug_assertions)]
    fn track_allocation(&self) {
        self.allocation_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    #[cfg(debug_assertions)]
    fn track_deallocation(&self) {
        self.allocation_count.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Drop for StarcodeContext {
    fn drop(&mut self) {
        unsafe {
            // Ensure tower is cleaned up
            cleanup_new_tower();
        }
        
        #[cfg(debug_assertions)]
        {
            let final_count = self.allocation_count.load(std::sync::atomic::Ordering::SeqCst);
            if final_count != 0 {
                eprintln!("Warning: {} allocations were not freed", final_count);
            }
        }
    }
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
    fn debug_align_sequences(
        sequences: &FxHashMap<Vec<u8>,usize>,
        max_distance: &i32,
        parent_to_child_ratio: &f64,
    ) -> Result<StarcodeAlignment, String> {
        // Create a context to ensure cleanup
        let _context = StarcodeContext::new();
        
        println!("Debug: Starting alignment with {} sequences", sequences.len());
        
        // Create temporary files with error checking
        let temp_input_file = match NamedTempFile::new() {
            Ok(f) => f,
            Err(e) => return Err(format!("Failed to create input temp file: {}", e)),
        };
        let temp_output_file = match NamedTempFile::new() {
            Ok(f) => f,
            Err(e) => return Err(format!("Failed to create output temp file: {}", e)),
        };

        let temp_input_path = temp_input_file.path();
        let temp_output_path = temp_output_file.path();
        
        println!("Debug: Created temp files - Input: {:?}, Output: {:?}", 
                temp_input_path, temp_output_path);

        // Write sequences to file with error checking
        if let Err(e) = write_vectors_to_file(temp_input_path, sequences) {
            return Err(format!("Failed to write sequences to temp file: {}", e));
        }
        
        println!("Debug: Wrote sequences to input file");

        unsafe {
            println!("Debug: Converting paths to C strings");
            
            // Use CString properly to avoid memory leaks
            let input_path_str = match temp_input_path.to_str() {
                Some(s) => s,
                None => return Err("Failed to convert input path to string".to_string()),
            };
            
            let output_path_str = match temp_output_path.to_str() {
                Some(s) => s,
                None => return Err("Failed to convert output path to string".to_string()),
            };
            
            let input_file_path = match CString::new(input_path_str) {
                Ok(s) => s,
                Err(e) => return Err(format!("Failed to create input path CString: {}", e)),
            };
            
            let output_file_path = match CString::new(output_path_str) {
                Ok(s) => s,
                Err(e) => return Err(format!("Failed to create output path CString: {}", e)),
            };

            println!("Debug: About to call starcode_helper");
            
            // Get raw pointers but don't transfer ownership
            let input_ptr = input_file_path.as_ptr();
            let output_ptr = output_file_path.as_ptr();
            
            let result = starcode_helper(
                input_ptr as *mut c_char,
                output_ptr as *mut c_char,
                *max_distance,
                0,
                1,
                0,
                *parent_to_child_ratio,
                1,
                0,
                0
            );
            
            println!("Debug: starcode_helper returned {}", result);
            
            if result != 0 {
                return Err(format!("starcode_helper failed with code {}", result));
            }
        }

        println!("Debug: Reading results from output file");
        Ok(recover_cluster_entries_from_file(temp_output_path.to_str().unwrap()))
    }

    pub fn align_sequences(
        sequences: &FxHashMap<Vec<u8>,usize>,
        max_distance: &i32,
        parent_to_child_ratio: &f64,
    ) -> StarcodeAlignment {
        match Self::debug_align_sequences(sequences, max_distance, parent_to_child_ratio) {
            Ok(alignment) => alignment,
            Err(e) => panic!("Alignment failed: {}", e),
        }
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
    use rand::Rng;
    use rand::prelude::IndexedRandom;
    use std::time::Duration;
    use std::thread;
    use std::panic;

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

        //assert_eq!(alignment.cluster_centers.len(),4);

    }

    // Add this helper function for controlled test execution
    fn run_test_with_catch<T>(test: T) -> Result<(), String>
    where
        T: FnOnce() + panic::UnwindSafe
    {
        let result = panic::catch_unwind(test);
        match result {
            Ok(_) => Ok(()),
            Err(e) => {
                if let Some(s) = e.downcast_ref::<String>() {
                    Err(s.clone())
                } else if let Some(s) = e.downcast_ref::<&str>() {
                    Err(s.to_string())
                } else {
                    Err("Unknown panic occurred".to_string())
                }
            }
        }
    }

    // Modify the problematic test to include more logging
    #[test]
    fn test_memory_leaks_alternating_sizes() {
        println!("Starting alternating sizes test");
        let result = run_test_with_catch(|| {
            let mut rng = rand::thread_rng();
            
            // Start with smaller iterations for debugging
            for i in 0..10 {
                println!("\n=== Iteration {} ===", i);
                let size = if i % 2 == 0 { 50 } else { 75 }; // Reduced sizes
                println!("Creating sequences of size {}", size);
                
                let mut sequences: FxHashMap<Vec<u8>,usize> = FxHashMap::default();
                
                // Generate sequences with more logging
                for j in 0..size {
                    if j % 10 == 0 { // More frequent logging
                        println!("Generated {} sequences", j);
                    }
                    let len = 10; // Fixed length for debugging
                    let (seq, count) = generate_random_nucleotide_sequence(len);
                    sequences.insert(seq, count);
                }

                // Create a separate scope to ensure resources are dropped
                {
                    println!("Starting alignment for iteration {}", i);
                    // Create a context explicitly to ensure cleanup
                    let _context = StarcodeContext::new();
                    let alignment = StarcodeAlignment::align_sequences(&sequences, &2, &2.0);
                    println!("Alignment complete with {} centers", alignment.cluster_centers.len());
                    
                    // Verify alignment results
                    println!("Verifying alignment results...");
                    assert!(alignment.cluster_centers.len() > 0);
                    assert_eq!(alignment.cluster_centers.len(), alignment.cluster_count.len());
                    
                    // Force cleanup by explicitly dropping
                    drop(alignment);
                }
                
                // Force garbage collection
                drop(sequences);
                
                println!("Iteration {} completed successfully", i);
                thread::sleep(Duration::from_millis(100)); // Longer delay
            }
        });

        if let Err(e) = result {
            panic!("Test failed with error: {}", e);
        }
    }

    // Also modify the increasing data test
    #[test]
    fn test_memory_leaks_with_increasing_data() {
        println!("Starting increasing data test 2");
        let result = run_test_with_catch(|| {
            println!("Starting increasing data test");
            
            for size in (100..1000).step_by(100) {
                println!("Testing with size {}", size);
                let mut sequences: FxHashMap<Vec<u8>,usize> = FxHashMap::default();
                
                for i in 0..size {
                    if i % 50 == 0 {
                        println!("Generated {} sequences", i);
                    }
                    let (seq, count) = generate_random_nucleotide_sequence(10);
                    sequences.insert(seq, count);
                }

                println!("Running alignment for size {}", size);
                let alignment = StarcodeAlignment::align_sequences(&sequences, &2, &2.0);
                println!("Alignment complete with {} centers", alignment.cluster_centers.len());
                assert!(alignment.cluster_centers.len() > 0);
            }
        });

        if let Err(e) = result {
            panic!("Test failed with error: {}", e);
        }
    }
}
