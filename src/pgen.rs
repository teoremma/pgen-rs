use std::collections::BTreeSet;
use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use evalexpr::{
  eval_boolean_with_context, eval_string_with_context, ContextWithMutableVariables,
  HashMapContext, Value,
};

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
    const VARIANT_BLOCK_SIZE: u64 = 1 << 16;

    pub fn from_file_path(file_path: String) -> io::Result<Pgen> {
        let file = File::open(file_path.clone())?;
        let mut buf_reader = BufReader::new(file);

        let current_position = buf_reader.stream_position().unwrap();
        assert_eq!(current_position, 0);

        let mut magic_number = [0u8; 2];
        buf_reader.read_exact(magic_number.as_mut())?;
        assert_eq!(magic_number, [0x6c, 0x1b]);

        let mut buf = [0u8; 1];
        buf_reader.read_exact(buf.as_mut())?;
        let storage_mode = buf[0];
        // for now we only deal with the standard storage mode
        // assert_eq!(storage_mode, 0x10);
        // print the storage mode in hex
        println!("storage mode: 0x{:x}", storage_mode);

        let mut buf = [0u8; 4];
        buf_reader.read_exact(buf.as_mut())?;
        let variant_count = u32::from_le_bytes(buf);
        println!("variant count: {}", variant_count);

        let mut buf = [0u8; 4];
        buf_reader.read_exact(buf.as_mut())?;
        let sample_count = u32::from_le_bytes(buf);
        println!("sample count: {}", sample_count);

        let mut buf = [0u8; 1];
        buf_reader.read_exact(buf.as_mut())?;
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

        // this should always be 12?
        let variant_block_start = buf_reader.stream_position().unwrap();
        assert_eq!(variant_block_start, pgen.variant_block_offsets_offset());

        let main_header_body_offset =
            pgen.check_variant_block_offsets(pgen.variant_block_offsets_offset());
        assert_eq!(main_header_body_offset, pgen.main_header_body_offset());

        let variant_records_offset = pgen.check_main_header_body(pgen.main_header_body_offset());
        assert_eq!(variant_records_offset, pgen.variant_records_offset());

        let passing_variants = pgen.filter_variants_with_expression("sum > 1500", &mut buf_reader).unwrap();
        println!("Passing variants: {:?}", passing_variants);


        Ok(pgen)
    }

    // Method to read and decode genotype data for a specific variant
    fn read_genotype_data(&self, variant_index: u64, reader: &mut BufReader<File>) -> io::Result<Vec<u8>> {
      // let file = File::open(&self.file_path)?;
      // let mut reader = BufReader::new(file);
      let offset = self.variant_records_offset();

      reader.seek(SeekFrom::Start(offset))?;

      // Assuming simple non-compressed storage for illustration; adapt as needed
      let num_samples = self.sample_count;
      let bytes_per_sample = match self.record_type_bits {
          4 => 1,  // Example: simple 2-bit genotype, packed in a single byte
          8 => 1,  // Each genotype uses one byte
          _ => panic!("Unsupported record type"),
      };

      let mut genotype_data = vec![0; num_samples as usize * bytes_per_sample];
      reader.read_exact(&mut genotype_data)?;

      Ok(genotype_data)
  }

  fn filter_variants_with_expression(&self, expression: &str, reader: &mut BufReader<File>) -> io::Result<Vec<u32>> {
    let mut passing_variants = Vec::new();
    for variant_index in 0..self.variant_count {
        let genotype_data = self.read_genotype_data(variant_index as u64, reader).unwrap();
        let context = self.create_context(variant_index as u64, &genotype_data);

        match eval_boolean_with_context(expression, &context) {
            Ok(result) if result => passing_variants.push(variant_index),
            Ok(_) => (),
            Err(e) => eprintln!("Failed to evaluate expression: {}", e),
        }
    }
    Ok(passing_variants)
  }

  fn create_context(&self, variant_index: u64, genotype_data: &[u8]) -> HashMapContext {
      let mut context = HashMapContext::new();
      // Assuming genotype_data is a Vec<u8> where each byte is a genotype like 0/1, 1/1, etc.
      context.set_value("variant_index".into(), Value::from(variant_index as i64)).unwrap();
      
      let sum: i64 = genotype_data.iter().map(|&gt| gt as i64).sum();
      let avg: f64 = sum as f64 / genotype_data.len() as f64;
      
      context.set_value("sum".into(), Value::from(sum)).unwrap();
      context.set_value("avg".into(), Value::from(avg)).unwrap();
      
      // Add more variables to the context as needed
      context
  }


    fn variant_block_count(&self) -> u64 {
        (self.variant_count as u64 + Pgen::VARIANT_BLOCK_SIZE - 1) / Pgen::VARIANT_BLOCK_SIZE
    }

    fn variant_block_offsets_offset(&self) -> u64 {
        12
    }

    fn variant_block_offsets_size(&self) -> u64 {
        self.variant_block_count() * 8
    }

    fn main_header_body_offset(&self) -> u64 {
        self.variant_block_offsets_offset() + self.variant_block_offsets_size()
    }

    fn main_header_body_size(&self) -> u64 {
        // the record type can be either 4 or 8 bits
        // the record length can be 1, 2, 3, or 4 bytes

        // the number of bytes needed to store all the record types
        let mut all_record_types_size = self.variant_count as u64 * self.record_type_bits as u64;
        if all_record_types_size % 8 != 0 {
            all_record_types_size += 4;
        }
        // this should be the case since the record type bits can only be 4 or 8
        assert!(all_record_types_size % 8 == 0);
        all_record_types_size /= 8;

        // the number of bytes needed to store all the record lengths
        let all_record_lengths_size = self.variant_count as u64 * self.record_length_bytes as u64;

        all_record_types_size + all_record_lengths_size
    }

    fn variant_records_offset(&self) -> u64 {
        self.main_header_body_offset() + self.main_header_body_size()
    }

    // checks that the given reader is at the correct position to read the variant block offsets
    fn check_variant_block_offsets(&self, offset: u64) -> u64 {
        let file = File::open(self.file_path.clone()).unwrap();
        let mut reader = BufReader::new(file);
        _ = reader.seek(SeekFrom::Start(offset)).unwrap();

        // right now we are reading all this to memory
        // we might want to to this in chunks
        let mut buf = [0u8; 8];
        let variant_block_offsets = (0..self.variant_block_count())
            .map(|_| {
                reader.read_exact(buf.as_mut()).unwrap();
                u64::from_le_bytes(buf)
            })
            .collect::<Vec<u64>>();
        println!("first variant block offset: {}", variant_block_offsets[0]);
        println!(
            "last variant block offset: {}",
            variant_block_offsets[variant_block_offsets.len() - 1]
        );

        let ascending = variant_block_offsets
            .windows(2)
            .all(|window| window[0] < window[1]);
        if !ascending {
            panic!("variant block offsets are not in ascending order")
        }

        // return the current position of the reader
        reader.stream_position().unwrap()
    }

    // checks the main header body of the pgen file starting at the given reader position
    fn check_main_header_body(&self, offset: u64) -> u64 {
        let n_blocks = self.variant_block_count();
        // each block consists of a packed array of record types followed by a packed array of record lengths
        // the record type sizes are determined by the record type bits
        // the record length sizes are determined by the record length bytes

        // read all the blocks and store the different record types and lengths using a set
        // just to get a sense of the distribution of record types and lengths
        let mut record_types: BTreeSet<u8> = BTreeSet::new();
        let mut record_lengths: BTreeSet<u64> = BTreeSet::new();
        // we need a reader for the record types and a reader for the record lengths

        let file = File::open(self.file_path.clone()).unwrap();
        let mut types_reader = BufReader::new(file);
        _ = types_reader.seek(SeekFrom::Start(offset)).unwrap();
        let file = File::open(self.file_path.clone()).unwrap();
        let mut lengths_reader = BufReader::new(file);
        _ = lengths_reader.seek(SeekFrom::Start(offset)).unwrap();

        // types_reader and lengths_reader should be at the same position at the beginning
        // of each iteration of the loop
        for block in 0..n_blocks {
            assert_eq!(
                types_reader.stream_position().unwrap(),
                lengths_reader.stream_position().unwrap()
            );
            // the number of variants should always be the same for each block
            // except for the last block which might have fewer variants
            let block_variant_count = if block == n_blocks - 1 {
                self.variant_count as u64 % Pgen::VARIANT_BLOCK_SIZE
            } else {
                Pgen::VARIANT_BLOCK_SIZE
            };

            // the number of bytes needed to store all the record types
            let mut types_block_size = block_variant_count * self.record_type_bits as u64;
            if types_block_size % 8 != 0 {
                types_block_size += 4;
            }
            assert!(types_block_size % 8 == 0);
            types_block_size /= 8;

            let lengths_block_size = block_variant_count * self.record_length_bytes as u64;

            // move the lenghts reader ahead by the size of the types block
            _ = lengths_reader
                .seek(SeekFrom::Current(types_block_size as i64))
                .unwrap();

            // read the record types and store them in the set
            let mut buf = vec![0u8; types_block_size as usize];
            types_reader.read_exact(buf.as_mut()).unwrap();
            // if the record type bits is 4, then we need to get both the high and low bits
            // if the record type bits is 8, then we store the whole byte
            for byte in buf {
                if self.record_type_bits == 4 {
                    record_types.insert(byte >> 4);
                    record_types.insert(byte & 0b1111);
                } else {
                    record_types.insert(byte);
                }
            }

            // read the record lengths and store them in the set
            let mut buf = vec![0u8; lengths_block_size as usize];
            lengths_reader.read_exact(buf.as_mut()).unwrap();
            for byte in buf {
                record_lengths.insert(byte as u64);
            }

            // move the types reader to the beginning of the next block,
            // which is the position of the lengths reader
            let lengths_reader_position = lengths_reader.stream_position().unwrap();
            _ = types_reader
                .seek(SeekFrom::Start(lengths_reader_position))
                .unwrap();
        }

        // println!("record types: {:?}", record_types);
        // print the record types as binary
        for record_type in record_types {
            println!("{:08b}", record_type);
        }

        // return the current position of the reader
        lengths_reader.stream_position().unwrap()
    }
}