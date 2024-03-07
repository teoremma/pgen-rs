use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Queries the pgen, outputting to stdout.
    ///
    /// All expressions have as variables the metadata being queried. For
    /// example, if querying the variants, CHROM and ID are variables which
    /// contain their respective values. This applies both for the expressions
    /// in the fstring and query.
    Query {
        /// The prefix of the pgen file triples. There should be three files
        /// PFILE_PREFIX.pgen, PFILE_PREFIX.psam, and PFILE_PREFIX.pvar.
        pfile_prefix: String,

        #[arg(short = 'f', long = "fstring")]
        /// An expression specifying what to output to stdout.
        query_fstring: String,

        #[arg(short = 'i', long = "include")]
        /// An expression specifying which variants (default) or samples (if -s
        /// is passed) to keep.
        query: Option<String>,
        #[arg(short = 's', long = "samples")]
        /// When passed, the query is over the samples. Otherwise it is over the
        /// variants. Defaults false.
        query_samples: bool,
    },
    /// Filters the pgen, outputting to a VCF.
    /// 
    /// All expressions have as variables the variant metadata. For example, if
    /// querying the variants, CHROM and ID are variables which contain their
    /// respective values.
    Filter {
        /// The prefix of the pgen file triples. There should be three files
        /// PFILE_PREFIX.pgen, PFILE_PREFIX.psam, and PFILE_PREFIX.pvar.
        pfile_prefix: String,

        #[arg(long = "include-var")]
        /// An expression specifying which variants to keep. If not passed,
        /// keeps all variants.
        var_query: Option<String>,

        #[arg(long = "include-sam")]
        /// An expression specifying which samples to keep. If not passed,
        /// keeps all samples.
        sam_query: Option<String>,

        #[arg(short = 'o', long = "out")]
        /// The output file name (defaults to PFILE_PREFIX.pgen-rs.vcf)
        out_file: Option<PathBuf>,
    }
}
