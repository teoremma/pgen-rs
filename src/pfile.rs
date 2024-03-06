use csv::{Reader, ReaderBuilder, StringRecord};
use evalexpr::{eval_boolean_with_context, ContextWithMutableVariables, HashMapContext, Value};
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write};

// use polars_core::prelude::*;
// use polars_io::prelude::*;

// use polars::prelude::*;
// use polars::prelude::{CsvReader, DataFrame, SerReader};
// use polars_io::RowIndex;
// use polars::prelude::DataFrame;

pub struct Pfile {
    pub pfile_prefix: String,
    pub num_variants: u32,
    pub num_samples: u32,
}

impl Pfile {
    pub fn pgen_path(&self) -> String {
        format!("{}.pgen", self.pfile_prefix)
    }

    pub fn psam_path(&self) -> String {
        format!("{}.psam", self.pfile_prefix)
    }

    pub fn pvar_path(&self) -> String {
        format!("{}.pvar", self.pfile_prefix)
    }

    pub fn from_prefix(pfile_prefix: String) -> Pfile {
        let pgen_path = format!("{}.pgen", pfile_prefix);

        let pgen = File::open(&pgen_path).unwrap();
        let mut pgen_reader = BufReader::new(pgen);

        let mut buf = [0u8; 2];
        pgen_reader.read_exact(&mut buf).unwrap();
        // check the magic number
        assert_eq!(buf, [0x6C, 0x1B]);

        let mut buf = [0u8; 1];
        pgen_reader.read_exact(&mut buf).unwrap();
        let storage_mode = buf[0];
        // for now we only deal with the simplest fixed-width storage mode
        assert!(storage_mode == 0x02);

        let mut buf = [0u8; 4];
        pgen_reader.read_exact(&mut buf).unwrap();
        let num_variants = u32::from_le_bytes(buf);
        println!("variant_count: {}", num_variants);

        let mut buf = [0u8; 4];
        pgen_reader.read_exact(&mut buf).unwrap();
        let num_samples = u32::from_le_bytes(buf);
        println!("sample_count: {}", num_samples);

        let mut buf = [0u8; 1];
        pgen_reader.read_exact(&mut buf).unwrap();
        // print this bit in binary
        // println!("flags: {:08b}", buf[0]);
        assert_eq!(buf, [0x40]);

        Pfile {
            pfile_prefix,
            num_variants,
            num_samples,
        }
    }

    pub fn output_vcf(&self, sam_query: Option<String>, var_query: Option<String>, var_batch_size: usize) -> csv::Result<()> {
        let (pvar_header, pvar_column_names) = self.read_pvar_header();
        let mut psam_reader = self.psam_reader()?;
        let sam_header = psam_reader.headers()?;
        // Index of the sample id in each sample record.
        let sam_rcd_id_idx = sam_header.iter().enumerate().find_map(|(idx, col)| {
            // TODO: make this a constant
            if col == "IID" {
                Some(idx)
            } else {
                None
            }
        }).expect(&format!("IID not among the headers of {}", self.psam_path()));
        let var_idx_rcd_batches = self.filter_metadata(&mut self.pvar_reader()?, var_query, var_batch_size)?;
        let sam_idx_rcd_batches = self.filter_metadata(&mut psam_reader, sam_query, 1)?;
        println!("filtered metadata");
        let sam_ids = sam_idx_rcd_batches
            .iter()
            .flat_map(|batch| {
                batch.iter().map(|(_idx, rcd)|
                                 rcd.get(sam_rcd_id_idx).unwrap().to_string()
                )
            })
            .collect::<Vec<String>>()
            .join("\t");

        let output_vcf_path = format!("{}.pgen-rs.vcf", self.pfile_prefix);
        let vcf = File::create(output_vcf_path).unwrap();
        let mut vcf_writer = BufWriter::new(vcf);
        // write the header
        write!(vcf_writer, "##fileformat=VCFv4.2\n").unwrap();
        write!(vcf_writer, "##source=pgen-rs\n").unwrap();
        write!(vcf_writer, "{}", pvar_header).unwrap();

        // avoid push_str since it is slow
        let pvar_column_names = pvar_column_names.trim().to_string();
        write!(vcf_writer, "{}", pvar_column_names).unwrap();
        write!(vcf_writer, "\tFORMAT\t{}\n", &sam_ids).unwrap();

        // now the fun part, write the actual data
        let pgen = File::open(&self.pgen_path()).unwrap();
        // seems that BufReader makes things slower
        // let mut pgen_reader = BufReader::new(pgen);
        let mut pgen_reader = pgen;
        for var_idx_rcd_batch in var_idx_rcd_batches.iter() {
            // Batches must not be empty.
            let (first_var_idx, _) = var_idx_rcd_batch[0];
            let record_offset = 12 + (first_var_idx as u32 * self.variant_record_size()) as u64;
            let mut record_buf = vec![0u8; self.variant_record_size() as usize * var_batch_size];
            pgen_reader.seek(SeekFrom::Start(record_offset)).unwrap();
            // println!("reading record at {}", record_offset);
            pgen_reader.read(&mut record_buf).unwrap();
            for (var_idx, var_rcd) in var_idx_rcd_batch.iter() {
                let pvar_line = var_rcd.iter().map(|s| s.to_string()).collect::<Vec<String>>().join("\t");
                write!(vcf_writer, "{}\tGT", pvar_line).unwrap();
                // pvar_line.push_str("\tGT");
                // write!(vcf_writer, "\tGT").unwrap();

                let var_offset = (var_idx - first_var_idx) * self.variant_record_size() as usize;
                // println!("reading record starting at {}", record_offset + var_offset as u64);
                // read the whole record to file
                // this restricts the number of syscalls to |variants| instead of |variants| * |samples|
                for (sam_idx, _sam_rcd) in sam_idx_rcd_batches.iter().flatten() {
                    let sample_offset = sam_idx / 4;
                    let host_byte = record_buf[var_offset + sample_offset as usize];
                    let in_byte_offset = sam_idx % 4;
                    let encoded_genotype = (host_byte >> (in_byte_offset * 2)) & 0b11;
    //
                    let genotype = match encoded_genotype {
                        0b00 => "0/0",
                        0b01 => "0/1",
                        0b10 => "1/1",
                        0b11 => "./.",
                        _ => panic!("unexpected genotype"),
                    };
                    // pvar_line.push_str("\t");
                    // pvar_line.push_str(genotype);
                    write!(vcf_writer, "\t{}", genotype).unwrap();

                }
                write!(vcf_writer, "\n").unwrap();
            }
            // pvar_line.push_str("\n");
            // write!(vcf_writer, "{}", pvar_line).unwrap();
        }
        Ok(())
    }

    fn variant_record_size(&self) -> u32 {
        let bit_size = self.num_samples * 2;
        let byte_size = (bit_size / 8) + if bit_size % 8 == 0 { 0 } else { 1 };
        byte_size
    }

    fn read_pvar_header(&self) -> (String, String) {
        let pvar = File::open(&self.pvar_path()).unwrap();
        let mut pvar_reader = BufReader::new(pvar);
        // read all lines that start with # and store them in a vector
        let mut header_lines = Vec::new();
        loop {
            let mut buf = String::new();
            pvar_reader.read_line(&mut buf).unwrap();
            if buf.starts_with("#") {
                header_lines.push(buf);
            } else {
                break;
            }
        }
        // the last line must be the column names
        let header = header_lines.pop().unwrap();
        // return the header comments and the column names
        (header_lines.join(""), header)
    }

    /// Gives the offset to the start of the headers without the headers'
    /// comment prefix.
    ///
    /// The pvar file will look like the following
    /// (psams look the same with different column types)
    ///
    ///     ## Some headers
    ///     ## ...
    ///     ## Other headers
    ///     ## Other headers
    ///     #CHROM ID POS ...
    ///     12 id pos ...
    ///     11 id pos ...
    ///
    /// Per the spec, the last header line will indicate the headers for
    /// the rest of the pvar file, in the case of the example this would be
    ///
    ///     #CHROM ID POS ...
    ///
    /// We want to seek to just after the # in that line so that we can
    /// give the file reader to csv and it'll handle parsing everything.
    ///
    ///     #CHROM ID POS ...
    ///      ^
    ///      |
    ///      start here
    fn find_metadata_file_header_start(meta_file: String) -> io::Result<u64> {
        let meta_file = File::open(meta_file)?;
        let mut meta_raw_reader = BufReader::new(meta_file);
        let mut prev_buf = String::new();
        let mut buf = String::new();
        loop {
            prev_buf = buf;
            buf = String::new();
            meta_raw_reader.read_line(&mut buf)?;
            // We are reading the data now
            if !buf.starts_with("#") {
                let current_pos = meta_raw_reader.seek(SeekFrom::Current(0))?;
                // The current line is not what we're looking for.
                // The header is the previous line, but it is forced to start
                // with a #.
                // We want to rewind to just after that #.
                let offset = (buf.len() + prev_buf.len()) as u64 - 1;
                return Ok(current_pos - offset);
            }
        }
    }

    fn metadata_file_reader(file: String, num_rows: usize) -> io::Result<Reader<File>> {
        let header_start = Pfile::find_metadata_file_header_start(file.clone())?;
        let mut meta_file = File::open(file)?;
        meta_file.seek(SeekFrom::Start(header_start))?;

        let meta_reader = ReaderBuilder::new()
            .delimiter(b'\t')
            // per the spec, there are no comments
            .buffer_capacity(num_rows)
            // we seek to exactly where the headers start
            .has_headers(true)
            .from_reader(meta_file);
        return Ok(meta_reader);
    }

    pub fn pvar_reader(&self) -> io::Result<Reader<File>> {
        Pfile::metadata_file_reader(self.pvar_path(), self.num_variants as usize)
    }

    pub fn filter_test(&self) -> io::Result<()> {
        let mut reader = self.pvar_reader()?;
        let headers: StringRecord = reader.headers()?.clone();
        let mut count = 0;
        for record in reader.records() {
            let mut context = HashMapContext::new();
            for (var, val) in std::iter::zip(&headers, &record?) {
                context
                    .set_value(var.to_string(), Value::String(val.to_string()))
                    .unwrap();
            }
            if eval_boolean_with_context("ID == \"rs8100066\"", &context).unwrap() {
                count += 1;
            }
        }
        println!("Keeping {} rows", count);
        Ok(())
    }

    pub fn psam_reader(&self) -> io::Result<Reader<File>> {
        Pfile::metadata_file_reader(self.psam_path(), self.num_samples as usize)
    }

    fn filter_metadata(&self, meta_reader: &mut Reader<File>, query: Option<String>, batch_size: usize) -> csv::Result<Vec<Vec<(usize, StringRecord)>>> {
        let headers: StringRecord = meta_reader.headers()?.clone();
        // NOTE: we could set the capacity of these, but we don't know ahead of
        // time how filled they will get.
        let mut kept_idx_vars_batches = Vec::new();
        let mut kept_idx_vars_batch = Vec::new();
        // This is for batching our reads of rows/cols, so each "batch"
        // corresponds to batch_size many records contiguous in memory.
        // Therefore, the maximum size of each of the batches is batch_size. We
        // however expect that there will generally be fewer than this number
        // unless every single record is matched.
        let mut batch_count = 0;
        for (idx, rcd) in meta_reader.records().enumerate() {
            let rcd = rcd?;
            let query_res = query.as_ref().map_or(true, |query| {
                let mut context = HashMapContext::new();
                for (var, val) in std::iter::zip(&headers, &rcd) {
                    context
                        .set_value(var.to_string(), Value::String(val.to_string()))
                        .unwrap();
                }
                eval_boolean_with_context(query, &context).unwrap()
            });
            if query_res {
                kept_idx_vars_batch.push((idx, rcd));
            }
            batch_count += 1;
            // Note how we don't compare the siez of the batches themselves, but
            // instead check against the batch_count variable, which always
            // counts up.
            if batch_count == batch_size {
                if !kept_idx_vars_batch.is_empty() {
                    kept_idx_vars_batches.push(kept_idx_vars_batch);
                }
                kept_idx_vars_batch = Vec::new();
                batch_count = 0;
            }
        }
        if !kept_idx_vars_batch.is_empty() {
            kept_idx_vars_batches.push(kept_idx_vars_batch);
        }
        Ok(kept_idx_vars_batches)
    }

}
