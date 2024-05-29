mod cli;
mod pfile;
mod pvar_parser;
mod filter_parser;

use actix_web::{web, HttpResponse, Responder};
use clap::Parser;
use cli::{Cli, Commands};
use pfile::Pfile;

use serde::{Deserialize, Serialize};
use shellwords::split;
use std::error::Error as StdError;

async fn index() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html")
        .body(include_str!("index.html"))
}

async fn styles() -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/css")
        .body(include_str!("styles.css"))
}

async fn scripts() -> impl Responder {
    HttpResponse::Ok()
        .content_type("application/javascript")
        .body(include_str!("scripts.js"))
}

#[derive(Deserialize)]
enum QueryType {
    Variant,
    Sample,
}

#[derive(Deserialize)]
struct FetchAIRequest {
    pfile_prefix: String,
    prompt: String,
    query_type: QueryType,
}

async fn fetch_ai_response(req_body: web::Json<FetchAIRequest>) -> impl Responder {
    // Read the secret from the environment variable
    let api_key = std::env::var("OPENAI_KEY")
        .expect("SECRET_KEY must be set in .env file or environment variable");
    // Call the fetch_response_from_ai function with the provided prompt and API key
    match fetch_response_from_ai(
        &req_body.pfile_prefix,
        &req_body.query_type,
        &req_body.prompt,
        &api_key,
    )
    .await
    {
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
                pfile
                    .query_metadata(&mut reader, query, query_fstring)
                    .unwrap();
            } else {
                let mut reader = pfile.pvar_reader().unwrap();
                pfile
                    .query_metadata(&mut reader, query, query_fstring)
                    .unwrap();
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

    // Load environment variables from the .env file
    dotenv::dotenv().ok();

    // Start Actix-web server to serve the HTML page and handle API requests
    actix_web::rt::System::new().block_on(async {
        let server = actix_web::HttpServer::new(|| {
            actix_web::App::new()
                .route("/", actix_web::web::get().to(index))
                .route("/styles.css", web::get().to(styles))
                .route("/scripts.js", web::get().to(scripts))
                .route(
                    "/fetch_ai_response",
                    actix_web::web::post().to(fetch_ai_response),
                )
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

async fn fetch_response_from_ai(
    pfile_prefix: &str,
    query_type: &QueryType,
    prompt: &str,
    api_key: &str,
) -> Result<String, Box<dyn StdError>> {
    let pfile = Pfile::from_prefix(pfile_prefix.to_string());
    let full_prompt = pfile.create_ai_query(query_type, prompt)?;
    let client = reqwest::Client::new();
    let request_body = OpenAIRequest {
        messages: vec![Message {
            role: "user".to_string(),
            content: full_prompt,
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
    println!("{}", response_text);

    if let Some(choices) = response.get("choices") {
        if let Some(choice) = choices.as_array().and_then(|arr| arr.first()) {
            if let Some(message) = choice
                .get("message")
                .and_then(|msg| msg.get("content").and_then(|c| c.as_str()))
            {
                return Ok(message.to_string());
            }
        }
    }

    Err("No valid choices found in API response".into())
}
