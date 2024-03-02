mod pgen;

use pgen::Pgen;

fn main() {
    let test_pgens = vec![
        "data/sample1/1000G_chr19_pruned.pgen",
        "data/sample2/pset1_1000G_chr16.pgen",
        "data/random1/random1.pgen",
    ];
    for pgen_path in test_pgens {
        println!("testing file: {}", pgen_path);
        let pgen = Pgen::from_file_path(pgen_path.to_string()).unwrap();
        println!("\n");
    }
}
