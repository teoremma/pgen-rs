use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};

pub struct Pgen {
    // magic_number: [u8; 2],
    file_path: String,
    storage_mode: u8,
    variant_count: u32,
    sample_count: u32,
    record_type_bits: u8,
    record_length_bytes: u8,
    allele_count_bytes: u8,
    provisional_ref_storage: u8,
    // format_byte: u8,
}

impl Pgen {
    const VARIANT_BLOCK_SIZE: u32 = 1 << 16;

    pub fn from_file_path(file_path: String) -> io::Result<Pgen> {
        let mut file = File::open(file_path.clone())?;

        let current_position = file.stream_position().unwrap();
        assert_eq!(current_position, 0);

        let mut magic_number = [0u8; 2];
        file.read_exact(magic_number.as_mut())?;
        assert_eq!(magic_number, [0x6c, 0x1b]);

        let mut buf = [0u8; 1];
        file.read_exact(buf.as_mut())?;
        let storage_mode = buf[0];
        // for now we only deal with the standard storage mode
        assert_eq!(storage_mode, 0x10);

        let mut buf = [0u8; 4];
        file.read_exact(buf.as_mut())?;
        let variant_count = u32::from_le_bytes(buf);
        println!("variant count: {}", variant_count);

        let mut buf = [0u8; 4];
        file.read_exact(buf.as_mut())?;
        let sample_count = u32::from_le_bytes(buf);
        println!("sample count: {}", sample_count);

        let mut buf = [0u8; 1];
        file.read_exact(buf.as_mut())?;
        let header_format_byte = buf[0];

        // The first 4 bits indicate the storage sizes
        let record_storage_mode = header_format_byte & (0b1111);
        let allele_count_bytes = (header_format_byte & (0b11 << 4)) >> 4;
        let provisional_ref_storage = (header_format_byte & (0b11 << 6)) >> 6;
        assert_eq!(provisional_ref_storage, 0b01);

        let record_type_bits: u8;
        match record_storage_mode / 4 {
            0 => record_type_bits = 4,
            1 => record_type_bits = 8,
            _ => panic!("invalid record storage mode"),
        }

        let record_length_bytes: u8 = record_storage_mode % 4 + 1;

        // println!("record storage mode {}", record_storage_mode);
        // println!("record type bits {}", record_type_bits);
        // println!("record length bytes {}", record_length_bytes);
        // println!("bytes per allele count {}", allele_count_bytes);
        // println!("provisional ref storage {}", provisional_ref_storage);

        // this should always be 12?
        let variant_block_start = file.stream_position().unwrap();
        assert_eq!(variant_block_start, 12);
        
        let pgen = Pgen {
            file_path,
            storage_mode,
            variant_count,
            sample_count,
            record_type_bits,
            record_length_bytes,
            allele_count_bytes,
            provisional_ref_storage,
        };

        pgen.check_variant_block_offsets();

        Ok(pgen)
    }

    fn variant_block_count(&self) -> u32 {
        (self.variant_count + Pgen::VARIANT_BLOCK_SIZE - 1) / Pgen::VARIANT_BLOCK_SIZE
    }

    fn variant_block_offsets_offset(&self) -> u32 {
        12
    }

    fn header_body_offset(&self) -> u32 {
        self.variant_block_offsets_offset() + self.variant_block_count() * 8
    }

    fn check_variant_block_offsets(&self) {
        let mut file = File::open(&self.file_path).unwrap();
        file.seek(SeekFrom::Start(self.variant_block_offsets_offset() as u64)).unwrap();
        let mut buf = [0u8; 8];
        let variant_block_offsets = (0..self.variant_block_count()).map(|_| {
            file.read_exact(buf.as_mut()).unwrap();
            u64::from_le_bytes(buf)
        }).collect::<Vec<u64>>();
        println!("first variant block offset: {}", variant_block_offsets[0]);
        println!("last variant block offset: {}", variant_block_offsets[variant_block_offsets.len() - 1]);

        let ascending = variant_block_offsets.windows(2).all(|window| window[0] < window[1]);
        if !ascending {
            panic!("variant block offsets are not in ascending order")
        }
    }
}