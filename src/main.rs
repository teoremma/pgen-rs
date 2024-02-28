mod reader;

fn main() {
    let test_pgens = vec![
        "data/sample1/1000G_chr19_pruned.pgen",
        "data/sample2/pset1_1000G_chr16.pgen",
        "data/random1/random1.pgen",
    ];
    for pgen_path in test_pgens {
        println!("testing file: {}", pgen_path);
        reader::read_header(pgen_path).unwrap();
        println!("\n");
    }
}
