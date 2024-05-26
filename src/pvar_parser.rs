use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "pvar_parser.pest"]
pub struct PvarParser;


fn main() {
    let successful_parse = PvarParser::parse(Rule::FILTER, "PASS");
    println!("{:?}", successful_parse);

    let unsuccessful_parse = PvarParser::parse(Rule::REF, "somethingelse");
    println!("{:?}", unsuccessful_parse);
}
