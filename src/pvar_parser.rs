use std::fs::File;
use std::io::{self, BufRead};

use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "pvar_parser.pest"]
pub struct PvarParser;

impl PvarParser {
    fn get_substring_from_index(input: &str, start_index: usize) -> Option<&str> {
        if start_index < input.len() {
            Some(&input[start_index..])
        } else {
            None
        }
    }
    
    fn get_idname(input: &str) -> Option<&str> {
        if let Some(successful_parse) = PvarParser::parse(Rule::infoheader, input).ok() {
            let parsed_str = successful_parse.as_str();
            let start_idx = 11;
            if let Some(substring) = Self::get_substring_from_index(parsed_str, start_idx) {
                return Some(substring);
            } else {
                return None;
            }
        } else {
            return None;
        }
    }
    
    fn get_idnames(filepath: &str) -> io::Result<Vec<String>> {
        let file_path = "data/basic1/basic1.pvar";
        let file = File::open(file_path)?;
        let reader = io::BufReader::new(file);
        let mut continue_parsing = true;
        let mut parsed_idnames = Vec::new();
    
        for line in reader.lines() {
            let line = line?;
            match Self::get_idname(&line) {
                Some(substr) => parsed_idnames.push(substr.to_string()),
                None => {
                    if line.contains("#CHROM") { // stop once we get to the real header
                        continue_parsing = false;
                        break;
                    }
                },
            }
            if !continue_parsing {
                break;
            }
        }
        Ok(parsed_idnames)
    }

    fn get_info_pairs(input: &str) -> Option<Vec<String>> {
        let pairs = PvarParser::parse(Rule::line, input).ok()?;
        for pair in pairs {
            for inner_pair in pair.into_inner() {
                if inner_pair.as_rule() == Rule::INFO {
                    let kv_pairs: Vec<String> = inner_pair.as_str().split(';').map(|s| s.to_string()).collect();
                    // let kv_pairs: Vec<&str> = inner_pair.as_str().split(';').collect();
                    return Some(kv_pairs);
                }
            }
        }
        None
    }
    
    fn get_info_line(input: &str) -> Option<&str> {
        let pairs = PvarParser::parse(Rule::line, input).ok()?;
        for pair in pairs {
            for inner_pair in pair.into_inner() {
                if inner_pair.as_rule() == Rule::INFO {
                    // let kv_pairs: Vec<String> = inner_pair.as_str().split(';').map(|s| s.to_string()).collect();
                    // // let kv_pairs: Vec<&str> = inner_pair.as_str().split(';').collect();
                    // return Some(kv_pairs);
                    return Some(inner_pair.as_str());
                }
            }
        }
        None
    }
    
    fn get_info_info(filepath: &str) -> io::Result<Vec<Vec<String>>> {
        let file_path = filepath;
        let file = File::open(file_path)?;
        let reader = io::BufReader::new(file);
        let mut parsed_kvpairs = Vec::new();
    
        for line in reader.lines() {
            let line = line?;
            if line.starts_with("#") {
                continue;
            }
            if let Some(kvpairs) = Self::get_info_pairs(&line) {
                parsed_kvpairs.push(kvpairs);
            }
        }
        Ok(parsed_kvpairs)
    }
    
    fn get_info_lines(filepath: &str) -> io::Result<Vec<String>> {
        let file_path = filepath;
        let file = File::open(file_path)?;
        let reader = io::BufReader::new(file);
        let mut parsed_kvpairs = Vec::new();
    
        for line in reader.lines() {
            let line = line?;
            if line.starts_with("#") {
                continue;
            }
            if let Some(kvpairs) = Self::get_info_line(&line) {
                parsed_kvpairs.push(kvpairs.to_string());
            }
        }
        Ok(parsed_kvpairs)
    }
    
    fn find_kv_pair<'a>(input: &'a str, prefix: &str) -> Option<&'a str> {
        if let Some(start_index) = input.find(prefix) {
            // println!("{:?}", start_index);
            let start_index = start_index + prefix.len();
            let end_index = input[start_index..].find(';')
                .map(|index| start_index + index)
                .unwrap_or_else(|| input.len());
            return Some(&input[start_index..end_index]);
            // if let Some(end_index) = input[start_index..].find(';') {
            //     return Some(&input[start_index..start_index + end_index]);
            // }
        }
        None
    }
    
    fn find_key<'a>(input: &'a str, prefix: &str) -> bool {
        if let Some(start_index) = input.find(prefix) {
            let start_index = start_index + prefix.len();
            let end_index = input[start_index..].find(';')
                .map(|index| start_index + index)
                .unwrap_or_else(|| input.len());
            let substr = &input[start_index..end_index];
            // if we've consumed the prefix and there's nothing else then we've found a single key
            return substr == "";
        } else {
            return false;
        }
    }

}




fn main() -> io::Result<()> {
    // let parsed_idnames = get_idnames("data/basic0.pvar");

    // for idname in &parsed_idnames {
    //     println!("{:?}", idname);
    // }

    // if let Some(info) = get_info_pairs("19	260912	rs8100066	G	A	100	PASS	AC=2731;AF=0.545327;AN=5008;NS=2504;DP=19168;EAS_AF=0.124;AMR_AF=0.5072;AFR_AF=0.8835;EUR_AF=0.6252;SAS_AF=0.4673;AA=.|||;VT=SNP") {
    //     println!("{:?}", info);
    // } else {
    //     println!("sad");
    // }

    // let info = get_info_info("data/basic0.pvar");
    // for inf in &info {
    //     for kv in inf {
    //         println!("{:?}", kv);
    //     }
    //     // println!("{:?}", inf)

    // }

    if let Some(info) = PvarParser::get_info_lines("data/basic1/basic1.pvar").ok() {
        let mut i = 0;
        for ln in info {
            // println!("{:?}", ln);
            // let prefix = format!("{}{}", "EAS_AF", "=");
            let prefix = "EX_TARGET";
            // if let Some(pair) = PvarParser::find_kv_pair(&ln, &prefix) {
            if PvarParser::find_key(&ln, &prefix) {
                println!("{}: {:?}", i, ln);
                // println!("{:?}", has);
                println!();
            }
            i += 1;
            // if i > 10 {
            //     break;
            // }
        }
    }


    Ok(())
}