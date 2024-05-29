use std::{io, iter::Filter};

use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "filter_parser.pest"]
pub struct FilterParser;


impl FilterParser {
    pub fn get_info_query(input: &str) -> Option<Vec<Vec<String>>> {
        let mut infokeys= Vec::new();
        let mut eqneq = String::new();
        let mut valstr = String::new();
        let mut found = false;
        let linepairs = FilterParser::parse(Rule::filterline, input).ok()?;
        for linepair in linepairs {
            for entrypair in linepair.into_inner() {
                if entrypair.as_rule() == Rule::filterentry {
                    for innerpair in entrypair.into_inner() {
                        match innerpair.as_rule() {
                            Rule::INFOQ => {
                                for keypair in innerpair.into_inner() {
                                    if keypair.as_rule() == Rule::key_only {
                                        infokeys.push(keypair.as_str().to_string());
                                    }
                                }
                                found = true;
                            },
                            Rule::eqneq => eqneq = innerpair.as_str().to_string(),
                            Rule::valid_entry => valstr = innerpair.as_str().to_string(),
                            _ => {}
                        }
                    }
                    if found {
                        let mut info_vals = Vec::new();
                        info_vals.push(infokeys);
                        let mut filter_vals = Vec::new();
                        filter_vals.push(eqneq);
                        filter_vals.push(valstr
                            .trim_start_matches(|c| c == '\'' || c == '"')
                            .trim_end_matches(|c| c == '\'' || c == '"')
                            .to_string());
                        
                        info_vals.push(filter_vals);
                        return Some(info_vals);
                    }
                }
            }
        }

        None
    }
}

fn main() -> io::Result<()> {

    if let Some(info) = FilterParser::get_info_query("POS == G and POS == 2 and INFO[u, v ] != '3'") {
        println!("{:?}", info);
    } else {
        println!("sad");
    }


    Ok(())
}
