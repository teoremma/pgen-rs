mod cli;
mod pfile;
mod pgen;

use clap::{Parser, Subcommand};
use cli::{Cli, Commands};
use pfile::Pfile;
use pgen::Pgen;
use std::io::{self, Write, ErrorKind::Other};
use std::error::Error as StdError;
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use actix_files as fs;
use actix_web::{web, App, HttpServer, HttpResponse, Responder};
use std::path::PathBuf;
use std::str::FromStr;
use shellwords::split;

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
//
// fn test_pfile() {
//     let pfile_prefix = "data/basic1/basic1";
//     let pfile = Pfile::from_prefix(pfile_prefix.to_string());
//     let mut pvar_reader = pfile.pvar_reader().unwrap();
//     println!("{:?}", pvar_reader.headers());
//     println!("{:?}", pvar_reader.records().next());
//     let mut psam_reader = pfile.psam_reader().unwrap();
//     println!("{:?}", psam_reader.headers());
//     println!("{:?}", psam_reader.records().next());
//     // pfile.filter_test();
//     let variant_ids = vec![
//         "rs8100066".to_string(),
//         "rs2312724".to_string(),
//         "rs1020382".to_string(),
//         "rs12459906".to_string(),
//         "rs7815".to_string(),
//     ];
//     let sample_ids = vec![
//         "HG00096".to_string(),
//         "HG00097".to_string(),
//         "HG00099".to_string(),
//         "HG00100".to_string(),
//         "HG00101".to_string(),
//     ];
//     pfile.output_vcf(Some("IID == \"HG00096\" || IID == \"HG00097\"".to_string()), Some("ID == \"rs2312724\" || ID == \"rs7815\"".to_string()));
// }
//
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

async fn index() -> HttpResponse {
  HttpResponse::Ok()
      .content_type("text/html")
      .body(include_str!("index.html"))
}

#[derive(Deserialize)]
struct FetchAIRequest {
    prompt: String,
}

async fn fetch_ai_response(req_body: web::Json<FetchAIRequest>) -> impl Responder {
    // Call the fetch_response_from_ai function with the provided prompt and API key
    match fetch_response_from_ai(&req_body.prompt, "FAKE_KEY").await {
        Ok(response) => HttpResponse::Ok().body(response),
        Err(err) => HttpResponse::InternalServerError().body(format!("Error: {:?}", err)),
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct SubmitQueryRequest {
    query: String, // Define the fields of the request as needed
}

async fn submit_query(req_body: web::Json<SubmitQueryRequest>) -> impl Responder {
  // Access the query from the request body
  let user_query = req_body.query.clone();
  println!("Received user query: {}", user_query);

  // Split the user query using shell parsing rules
  let user_query_parts = split(&user_query).unwrap();
  println!("PARTS {:?}", user_query_parts);
  let cli_result = Cli::try_parse_from(user_query_parts);

  println!("RESULT {:?}", cli_result);
  let cli = match cli_result {
      Ok(cli) => cli,
      Err(error) => {
          return HttpResponse::BadRequest().body(format!("Error parsing user query: {}", error));
      }
  };

  // Execute the corresponding command
  match cli.command {
    Some(Commands::Query {
        pfile_prefix,
        query_fstring,
        query,
        query_samples,
    }) => {
        let pfile = Pfile::from_prefix(pfile_prefix);
        if query_samples {
            let mut reader = pfile.psam_reader().unwrap();
            pfile.query_metadata(&mut reader, query, query_fstring).unwrap();
        } else {
            let mut reader = pfile.pvar_reader().unwrap();
            pfile.query_metadata(&mut reader, query, query_fstring).unwrap();
        }
    }
    Some(Commands::Filter {
        pfile_prefix,
        var_query,
        sam_query,
        out_file,
    }) => {
        let pfile = Pfile::from_prefix(pfile_prefix);
        let out_file =
            out_file.unwrap_or_else(|| format!("{}.pgen-rs.vcf", pfile.pfile_prefix).into());
        pfile.output_vcf(sam_query, var_query, out_file).unwrap();
    }
    None => {
        return HttpResponse::BadRequest().body("Invalid user query: No command provided");
    }
  };

  HttpResponse::Ok().finish() // Return a response indicating success
}


fn main() {
  // test_pgen();
      
  // Start Actix-web server to serve the HTML page and handle API requests
  actix_web::rt::System::new().block_on(async {
    let server = actix_web::HttpServer::new(|| {
        actix_web::App::new()
            .route("/", actix_web::web::get().to(index))
            .service(actix_files::Files::new("/static", "static").show_files_listing())
            .route("/fetch_ai_response", actix_web::web::post().to(fetch_ai_response))
            .route("/submit_query", actix_web::web::post().to(submit_query)) 
    })
    .bind("127.0.0.1:8080")
    .unwrap()
    .run();

    println!("Server running at http://127.0.0.1:8080");

    // Wait for the server to finish running
    let _ = server.await;
  });
}

#[derive(Serialize)]
struct OpenAIRequest {
    messages: Vec<Message>,
    max_tokens: usize,
    model: String,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: String,
}

async fn fetch_response_from_ai(prompt: &str, api_key: &str) -> Result<String, Box<dyn StdError>> {
  let client = reqwest::Client::new();
  let request_body = OpenAIRequest {
    messages: vec![Message {
      role: "user".to_string(),
      content: prompt.to_string(),
    }],
    max_tokens: 50,
    model: "gpt-3.5-turbo".to_string(),
  };

  let response = client
      .post("https://api.openai.com/v1/chat/completions")
      .header("Authorization", format!("Bearer {}", api_key))
      .json(&request_body)
      .send()
      .await?
      .json::<serde_json::Value>() // Deserialize response into serde_json::Value
      .await?;

  let response_text = response.to_string();

  if let Some(choices) = response.get("choices") {
    if let Some(choice) = choices.as_array().and_then(|arr| arr.get(0)) {
        if let Some(message) = choice.get("message").and_then(|msg| msg.get("content").and_then(|c| c.as_str())) {
            return Ok(message.to_string());
        }
    }
  }

  Err("No valid choices found in API response".into())
}
