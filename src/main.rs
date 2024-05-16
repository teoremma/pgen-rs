mod cli;
mod pfile;
mod pgen;

use clap::Parser;
use cli::{Cli, Commands};
use pfile::Pfile;
use pgen::Pgen;
use crossterm::{
  event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers},
  execute,
  terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use tui::{
  backend::TermionBackend,
  widgets::{Widget, List, ListItem, ListState, Block, Borders},
  layout::{Layout, Constraint, Direction},
  Terminal,
  Frame,
  style::{Style, Modifier, Color},
};
use termion::{input::MouseTerminal, raw::IntoRawMode, screen::AlternateScreen, screen::IntoAlternateScreen, event::Key, input::TermRead};
use std::io::{self, Write, ErrorKind::Other};
use std::error::Error as StdError;
use tokio::{runtime::Runtime, sync::mpsc};
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

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

fn main() {
  // test_pgen();
  
    let cli = Cli::parse();
  if cli.interactive {
    let rt = Runtime::new().unwrap();
        rt.block_on(async {
          enable_raw_mode().expect("Failed to enable raw mode");
          let (tx, rx) = mpsc::channel(100); // Changed to non-mutable rx
          let (signal_tx, mut signal_rx) = mpsc::channel(1); // Channel for signaling to start generate_items

            
            tokio::spawn(async move {
              run_tui(rx, signal_tx).await;
            });

            // If a signal is received, start generate_items
            while let Some(_) = signal_rx.recv().await {
              let api_key = "FAKE_KEY".to_string();
              generate_items(tx.clone(), api_key).await;
            }
        });
  } else if let Some(command) = cli.command {
    match command {
        Commands::Query {
            pfile_prefix,
            query_fstring,
            query,
            query_samples,
        } => {
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
        Commands::Filter {
            pfile_prefix,
            var_query,
            sam_query,
            out_file,
        } => {
            let pfile = Pfile::from_prefix(pfile_prefix);
            let out_file =
                out_file.unwrap_or_else(|| format!("{}.pgen-rs.vcf", pfile.pfile_prefix).into());
            pfile.output_vcf(sam_query, var_query, out_file).unwrap();
        }
    }
  }
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
  // println!("Response Text: {}", response_text);

  if let Some(choices) = response.get("choices") {
    if let Some(choice) = choices.as_array().and_then(|arr| arr.get(0)) {
        if let Some(message) = choice.get("message").and_then(|msg| msg.get("content").and_then(|c| c.as_str())) {
            return Ok(message.to_string());
        }
    }
  }

  Err("No valid choices found in API response".into())
}

async fn generate_items(mut sender: mpsc::Sender<Vec<ListItem<'static>>>, api_key: String) {
  let prompts = vec!["Please generate a query for genomic data using bcftools"];
  let mut items = Vec::new();

  for prompt in prompts {
    match fetch_response_from_ai(prompt, &api_key).await {
        Ok(response) => items.push(ListItem::new(response)),
        Err(err) => {
            eprintln!("Error fetching AI response: {:?}", err);
            items.push(ListItem::new("Error fetching AI response"));
        }
    }
}

  sender.send(items).await.unwrap();
}

#[derive(Clone)]
struct SharedState {
    user_input: Arc<Mutex<String>>,
}

async fn run_tui(mut rx: mpsc::Receiver<Vec<ListItem<'static>>>, signal_tx: mpsc::Sender<()>) {
  let mut stdout = io::stdout().into_alternate_screen().unwrap();
  let backend = TermionBackend::new(stdout);
  let mut terminal = Terminal::new(backend).unwrap();
  let stdin = io::stdin();
  let mut keys = stdin.keys();
  let user_input = Arc::new(Mutex::new(String::new()));
  let shared_state = SharedState {
      user_input: user_input.clone(),
  };
  let mut items = vec![];

  loop {
    terminal.draw(|f| {
      let chunks = Layout::default()
          .direction(Direction::Vertical)
          .margin(1)
          .constraints([Constraint::Min(1), Constraint::Length(3)].as_ref()) // Change here
          .split(f.size());

      // Render AI-generated queries
      let list = List::new(items.clone())
          .block(Block::default().borders(Borders::ALL))
          .highlight_style(Style::default().add_modifier(Modifier::BOLD).bg(Color::Blue))
          .highlight_symbol(">> ");

      let mut list_state = tui::widgets::ListState::default();
      f.render_stateful_widget(list, chunks[0], &mut list_state);

      // Render user input
      let user_input_style = Style::default().fg(Color::Yellow);
      let user_input_text = shared_state.user_input.lock().unwrap();
      f.render_widget(
          tui::widgets::Paragraph::new(user_input_text.as_str()).style(user_input_style),
          chunks[1], // Change here
      );
  }).unwrap();

      // Handle key presses
      if let Some(Ok(key)) = keys.next() {
          match key {
              Key::Char('q') => {
                  // println!("EXITING");
                  break;
              },
              Key::Char(' ') => {
                  // println!("SPACE");
                  // Send signal to start generate_items
                  // let mut user_input = shared_state.user_input.lock().unwrap();
                  // user_input.push(' '); 
                  if let Err(err) = signal_tx.send(()).await {
                      eprintln!("Error sending signal to start generate_items: {:?}", err);
                  }
              },
              Key::Char(c) => {
                  // println!("CHAR {}", c);
                  let mut user_input = shared_state.user_input.lock().unwrap();
                  user_input.push(c);
              }
              Key::Backspace => {
                  // println!("BACK");
                  let mut user_input = shared_state.user_input.lock().unwrap();
                  user_input.pop();
              }
              _ => {
                  // println!("Unrecognized key: {:?}", key);
              }
          }
      }

      // Check for new items from the channel
      match rx.try_recv() {
          Ok(new_items) => {
              // If receiving succeeds, update the items
              items = new_items;
          }
          Err(_) => {
              // If an error occurs or there are no new items, continue
          }
      }
  }

  // Disable raw mode
  disable_raw_mode().expect("Failed to disable raw mode");
}