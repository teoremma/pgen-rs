use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Queries the pgen, outputting to stdout a VCF (default) or custom format
    /// string.
    Query {
        /// The prefix of the pgen file triples. There should be three files
        /// PFILE_PREFIX.pgen, PFILE_PREFIX.psam, and PFILE_PREFIX.pvar.
        pfile_prefix: String,

        #[arg(long = "var-query")]
        /// Specifies a query on which variants to keep. If not passed, keeps all variants.
        var_query: Option<String>,

        #[arg(long = "sam-query")]
        /// Specifies a query on which samples to keep. If not passed, keeps all variants.
        sam_query: Option<String>,

        #[arg(long = "query-fstring")]
        /// Specifies what to output to stdout for each variant. If not passed, a VCF is output.
        query_fstring: Option<String>,

        #[arg(long = "var-batch-size", default_value = "1")]
        /// Batch size for how many variant rows to read at once when generating a VCF.
        var_batch_size: usize
    },
}
