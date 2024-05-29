use csv::{Reader, ReaderBuilder, StringRecord};
use evalexpr::{
    eval_boolean_with_context, eval_string_with_context, ContextWithMutableVariables,
    HashMapContext, Value,
};
use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::iter::Filter;
use std::path::PathBuf;

use crate::QueryType;
use crate::pvar_parser::PvarParser;
use crate::filter_parser::FilterParser;

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

        let pgen = File::open(pgen_path).unwrap();
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
        // println!("variant_count: {}", num_variants);

        let mut buf = [0u8; 4];
        pgen_reader.read_exact(&mut buf).unwrap();
        let num_samples = u32::from_le_bytes(buf);
        // println!("sample_count: {}", num_samples);

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


    fn parse_info_query(&self, query: String) -> Option<(Vec<String>, Vec<String>)> {
        let info = FilterParser::get_info_query(&query);
        match info {
            Some(info_vals) => {
                let info_keys = info_vals.first();
                let info_filtervals = info_vals.last();
                match (info_keys, info_filtervals) {
                    (Some(info_keys), Some(info_filtervals)) => {
                        if let Some(valid_info_keys) = PvarParser::get_meta_idnames(&self.pvar_path()).ok() {
                            if info_keys.iter().all(|item| valid_info_keys.contains(item)) {
                                return Some((info_keys.to_vec(), info_filtervals.to_vec()));
                            }
                        }

                    },
                    _ => (),
                };
            },
            None => (),
        };
        None
    }

    pub fn query_metadata(
        &self,
        reader: &mut Reader<File>,
        query: Option<String>,
        f_string: String,
    ) -> csv::Result<()> {
        let headers: StringRecord = reader.headers()?.clone();

        // if query.is_some() {
        //     if let Some((info_keys, info_filtervals)) = Self::parse_info_query(&self, query.clone().unwrap()) {

        //     } else {
        //         println!("invalid query: these fields don't exist in the INFO column");
        //     }
        // }

        for (_idx, rcd) in reader.records().enumerate() {
            let rcd = rcd?;
            let mut context = HashMapContext::new();
            for (var, val) in std::iter::zip(&headers, &rcd) {
                // add the individual values from the INFO column to the context
                if var == "INFO" {
                    let kvpairs = PvarParser::get_info_kv_pairs(val);
                    for (k, v) in kvpairs {
                        context.set_value(format!("{}{}{}","INFO[",k ,"]"), Value::String(v)).unwrap();

                    }
                }
                    context
                        .set_value(var.to_string(), Value::String(val.to_string()))
                        .unwrap();
            }
            let query_res = query.as_ref().map_or(true, |query| {
                eval_boolean_with_context(query, &context).unwrap()
            });

            if query_res {
                let output = eval_string_with_context(&f_string, &context).unwrap();
                println!("{}", output);
            }
        }
        Ok(())
    }

    pub fn output_vcf(
        &self,
        sam_query: Option<String>,
        var_query: Option<String>,
        filename: PathBuf,
    ) -> csv::Result<()> {
        let (pvar_header, pvar_column_names) = self.read_pvar_header();
        let mut psam_reader = self.psam_reader()?;
        let sam_header = psam_reader.headers()?;
        // Index of the sample id in each sample record.
        let sam_rcd_id_idx = sam_header
            .iter()
            .enumerate()
            .find_map(|(idx, col)| {
                // TODO: make this a constant
                if col == "IID" {
                    Some(idx)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| panic!("IID not among the headers of {}", self.psam_path()));
        let var_idx_rcds = self.filter_metadata(&mut self.pvar_reader()?, var_query)?;
        let sam_idx_rcs = self.filter_metadata(&mut psam_reader, sam_query)?;
        // println!("filtered metadata");
        let sam_ids = sam_idx_rcs
            .iter()
            .map(|(_idx, rcd)| rcd.get(sam_rcd_id_idx).unwrap().to_string())
            .collect::<Vec<String>>()
            .join("\t");
        let vcf = File::create(filename)?;
        let mut vcf_writer = BufWriter::new(vcf);
        // write the header
        writeln!(vcf_writer, "##fileformat=VCFv4.2").unwrap();
        writeln!(vcf_writer, "##source=pgen-rs").unwrap();
        write!(vcf_writer, "{}", pvar_header).unwrap();

        // avoid push_str since it is slow
        let pvar_column_names = pvar_column_names.trim().to_string();
        write!(vcf_writer, "{}", pvar_column_names).unwrap();
        writeln!(vcf_writer, "\tFORMAT\t{}", &sam_ids).unwrap();

        // now the fun part, write the actual data
        let pgen = File::open(self.pgen_path()).unwrap();
        // seems that BufReader makes things slower
        // let mut pgen_reader = BufReader::new(pgen);
        let mut pgen_reader = pgen;
        // For writing the hot part of the loop (the body of the VCF) we will
        // use BufWriter::write for performance reasons.
        // println!("wrote header");
        for (var_idx, var_rcd) in var_idx_rcds.iter() {
            for col in var_rcd.iter() {
                vcf_writer.write(col.as_bytes())?;
                vcf_writer.write(b"\t")?;
            }
            vcf_writer.write("GT".as_bytes())?;
            // pvar_line.push_str("\tGT");
            // write!(vcf_writer, "\tGT").unwrap();

            let record_offset = 12 + (*var_idx as u32 * self.variant_record_size()) as u64;
            // read the whole record to file
            // this restricts the number of syscalls to |variants| instead of |variants| * |samples|
            let mut record_buf = vec![0u8; self.variant_record_size() as usize];
            pgen_reader.seek(SeekFrom::Start(record_offset)).unwrap();
            pgen_reader.read_exact(&mut record_buf).unwrap();
            for (sam_idx, _sam_rcd) in sam_idx_rcs.iter() {
                let sample_offset = sam_idx / 4;
                let host_byte = record_buf[sample_offset];
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
                vcf_writer.write(b"\t")?;
                vcf_writer.write(genotype.as_bytes())?;
            }
            // pvar_line.push_str("\n");
            vcf_writer.write(b"\n")?;
            // write!(vcf_writer, "{}", pvar_line).unwrap();
        }
        Ok(())
    }

    fn variant_record_size(&self) -> u32 {
        let bit_size = self.num_samples * 2;

        (bit_size / 8) + if bit_size % 8 == 0 { 0 } else { 1 }
    }

    fn read_pvar_header(&self) -> (String, String) {
        let pvar = File::open(self.pvar_path()).unwrap();
        let mut pvar_reader = BufReader::new(pvar);
        // read all lines that start with # and store them in a vector
        let mut header_lines = Vec::new();
        loop {
            let mut buf = String::new();
            pvar_reader.read_line(&mut buf).unwrap();
            if buf.starts_with('#') {
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
        #[allow(unused_assignments)]
        let mut prev_buf = String::new();
        let mut buf = String::new();
        loop {
            prev_buf = buf;
            buf = String::new();
            meta_raw_reader.read_line(&mut buf)?;
            // We are reading the data now
            if !buf.starts_with('#') {
                let current_pos = meta_raw_reader.stream_position()?;
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
        Ok(meta_reader)
    }

    pub fn pvar_reader(&self) -> io::Result<Reader<File>> {
        Pfile::metadata_file_reader(self.pvar_path(), self.num_variants as usize)
    }

    fn _filter_test(&self) -> io::Result<()> {
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

    fn filter_metadata(
        &self,
        meta_reader: &mut Reader<File>,
        query: Option<String>,
    ) -> csv::Result<Vec<(usize, StringRecord)>> {
        let headers: StringRecord = meta_reader.headers()?.clone();
        let mut kept_idx_vars = Vec::new();
        for (idx, rcd) in meta_reader.records().enumerate() {
            let rcd = rcd?;
            let query_res = query.as_ref().map_or(true, |query| {
                let mut context = HashMapContext::new();
                for (var, val) in std::iter::zip(&headers, &rcd) {
                    if var == "INFO" {
                        let kvpairs = PvarParser::get_info_kv_pairs(val);
                        for (k, v) in kvpairs {
                            context.set_value(format!("{}{}{}","INFO[",k ,"]"), Value::String(v)).unwrap();
                        }
                    }
                    context
                        .set_value(var.to_string(), Value::String(val.to_string()))
                        .unwrap();
                }
                eval_boolean_with_context(query, &context).unwrap()
            });
            if query_res {
                kept_idx_vars.push((idx, rcd));
            }
        }
        Ok(kept_idx_vars)
    }

    /// Returns the columns and first row for the given metadata type.
    ///
    /// E.g. for .psam (with spaces intead of tabs):
    ///
    /// IID SEX
    /// id1 N/A
    pub fn metadata_columns_and_first_row(&self, query_type: &QueryType) -> io::Result<String> {
        let mut meta_reader = match query_type {
            QueryType::Sample => self.psam_reader(),
            QueryType::Variant => self.pvar_reader(),
        }?;
        let columns = meta_reader
            .headers()?
            .iter()
            .collect::<Vec<&str>>()
            .join("\t");
        let first_row = meta_reader
            .records()
            .next()
            .unwrap()?
            .iter()
            .collect::<Vec<&str>>()
            .join("\t");
        Ok(columns + "\n" + &first_row)
    }

    pub fn create_ai_query(&self, query_type: &QueryType, prompt: &str) -> io::Result<String> {
        match query_type {
            QueryType::Sample => {
                Ok(format!(r#"
I've developed a tool for filtering genomic data by sample and I would like you to help me write a query for it.

Queries are expressions that return a boolean value in a simple expression language. This language supports operations common to many programming languages, such as `==` for equality comparison, `""` for constructing string literals, and  `||` for boolean OR.

To make these queries not behave statically, the tool instantiates special variables with values from the data itself. All of the variables are of type string, even those which represent numeric data.

When filtering, the tool creates variables which correspond to the metadata fields in the sample, such as its identifier `IID`.

Below is a list of all variables as well as sample values for each. The first line corresponds to the variables. The second line is a sample value for each.

```
{}
```

Below are two sample queries:

Keep all of the samples with `SEX` not equal to `N/A`.

```
SEX != "N/A"
```

Keep all of the samples with ID `samp1`.

```
IID == "samp1"
```

I would like you to write me a query with the following specification: {}.

Please return a response with one query per line. If you are confident in it, only include one query; otherwise, include as many queries you think represent different interpretations or implementations of the original request. Do not include any additional explanation or formatting. Your response will be parsed and then executed to filter the data.
"#, self.metadata_columns_and_first_row(query_type)?, prompt).to_string())
            }
            QueryType::Variant => {
                Ok(format!(r#"
I've developed a tool for filtering genomic data by variant and I would like you to help me write a query for it.

Queries are expressions that return a boolean value in a simple expression language. This language supports operations common to many programming languages, such as `==` for equality comparison, `""` for constructing string literals, and  `||` for boolean OR.

To make these queries not behave statically, the tool instantiates special variables with values from the data itself. All of the variables are of type string, even those which represent numeric data.

When filtering, the tool creates variables which correspond to the standard values seen in a .vcf file. For example, `ALT` references to the alternate allele for the current variant. There may be other variables too.

Below is a list of all variables as well as sample values for each. The first line corresponds to the variables. The second line is a sample value for each.

```
{}
```

Below are two sample queries:

Keep all of the variants with `POS` equal to `10` or `20`.

```
POS=="10" || POS=="20"
```

Keep all of the variants with `G` as the alternate allele and whose `POS` isn't `10`.

```
ALT=="G" && POS !="10"
```

I would like you to write me a query with the following specification: {}.

Please return a response with one query per line. If you are confident in it, only include one query; otherwise, include as many queries you think represent different interpretations or implementations of the original request. Do not include any additional explanation or formatting. Your response will be parsed and then executed to filter the data.
"#, self.metadata_columns_and_first_row(query_type)?, prompt).to_string())
            }
        }
    }
}
