use std::fs::File;
use std::io::{self, Read};
// use std::io::Result;

// this function print an array or vec of u8 in hex format
pub fn print_hex(data: &[u8]) {
    for byte in data {
        print!("{:#02x} ", byte);
    }
    println!();
}

// This function reads the header of a PGEN file to check for validity
// The argument is a file path to the PGEN file
// The return value is a Result type, which is either an error or a Header struct
pub fn read_header(file_path: &str) -> io::Result<()> {
// pub fn read_header(file_path: &str) {
    // Open the file
    let mut file = File::open(file_path)?;

    let mut magic_number = [0u8; 2];
    file.read_exact(magic_number.as_mut())?;
    assert_eq!(magic_number, [0x6c, 0x1b]);
    println!("magic number ok");

    let mut storage_mode = [0u8; 1];
    file.read_exact(storage_mode.as_mut())?;
    let storage_mode = storage_mode[0];
    // for now we only deal with the standard storage mode
    assert_eq!(storage_mode, 0x10);
    println!("standard storage mode");

    let mut variant_count = [0u8; 4];
    file.read_exact(variant_count.as_mut())?;
    let variant_count = u32::from_le_bytes(variant_count);
    println!("variant count: {}", variant_count);

    let mut sample_count = [0u8; 4];
    file.read_exact(sample_count.as_mut())?;
    let sample_count = u32::from_le_bytes(sample_count);
    println!("sample count: {}", sample_count);

    let mut format_byte = [0u8; 1];
    file.read_exact(format_byte.as_mut())?;
    let format_byte = format_byte[0];
    println!("storage mode: {:08b}", format_byte);

    Ok(())
}