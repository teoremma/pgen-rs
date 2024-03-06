mod cli;
mod pfile;
mod pgen;

use clap::CommandFactory;
use clap::Parser;
use cli::{Cli, Commands};
use pfile::Pfile;
use pgen::Pgen;

fn test_pgen() {
    let test_pgens = vec![
        // "data/sample1/1000G_chr19_pruned.pgen",
        // "data/sample2/pset1_1000G_chr16.pgen",
        "data/random1/random1.pgen",
        // "data/random2/random2.pgen",
        "data/basic1/basic1.pgen",
    ];
    for pgen_path in test_pgens {
        println!("testing file: {}", pgen_path);
        let pgen = Pgen::from_file_path(pgen_path.to_string()).unwrap();
        println!("\n");
    }
}

fn test_pfile() {
    let pfile_prefix = "data/basic1/basic1";
    let pfile = Pfile::from_prefix(pfile_prefix.to_string());
    let mut pvar_reader = pfile.pvar_reader().unwrap();
    println!("{:?}", pvar_reader.headers());
    println!("{:?}", pvar_reader.records().next());
    let mut psam_reader = pfile.psam_reader().unwrap();
    println!("{:?}", psam_reader.headers());
    println!("{:?}", psam_reader.records().next());
    // pfile.filter_test();
    let variant_ids = vec![
        "rs8100066".to_string(),
        "rs2312724".to_string(),
        "rs1020382".to_string(),
        "rs12459906".to_string(),
        "rs7815".to_string(),
    ];
    let sample_ids = vec![
        "HG00096".to_string(),
        "HG00097".to_string(),
        "HG00099".to_string(),
        "HG00100".to_string(),
        "HG00101".to_string(),
    ];
    pfile.output_vcf(Some("IID == \"HG00096\" || IID == \"HG00097\"".to_string()), Some("ID == \"rs2312724\" || ID == \"rs7815\"".to_string()));
}

// fn test_pfile2() {
//     let pfile_prefix = "data/basic2/basic2";
//     let pfile = Pfile::from_prefix(pfile_prefix.to_string());
//     // let variant_ids = vec![
//     //     "snp2".to_string(),
//     //     "snp4".to_string(),
//     //     "snp8".to_string(),
//     // ];
//     // let sample_ids = vec![
//     //     "per2".to_string(),
//     //     "per4".to_string(),
//     //     "per8".to_string(),
//     // ];
//
//     let variant_ids = vec![
//         "snp0".to_string(),
//         "snp1".to_string(),
//         "snp2".to_string(),
//         "snp3".to_string(),
//         "snp4".to_string(),
//         "snp5".to_string(),
//         "snp6".to_string(),
//         "snp7".to_string(),
//         "snp8".to_string(),
//         "snp9".to_string(),
//     ];
//     let sample_ids = vec![
//         "per0".to_string(),
//         "per1".to_string(),
//         "per2".to_string(),
//         "per3".to_string(),
//         "per4".to_string(),
//         "per5".to_string(),
//         "per6".to_string(),
//         "per7".to_string(),
//         "per8".to_string(),
//         "per9".to_string(),
//     ];
//     pfile.output_vcf(sample_ids, variant_ids);
// }

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Query {
            pfile_prefix,
            var_query,
            sam_query,
            query_fstring,
        } => {
            let pfile = Pfile::from_prefix(pfile_prefix);
            if let Some(query_fstring) = query_fstring {
                pfile.query_vars(var_query, query_fstring).unwrap();
            } else {
                pfile.output_vcf(sam_query, var_query).unwrap();
            }
        }
    }
    // test_pfile2();
}
