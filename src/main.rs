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
use std::io::{self, Write};
use tokio::runtime::Runtime;

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
      // If interactive mode is enabled, run the TUI
      let rt = Runtime::new().unwrap();
      rt.block_on(run_tui());
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

async fn run_tui()  {
    let mut stdout = io::stdout().into_alternate_screen().unwrap();
    let backend = TermionBackend::new(stdout);
    let mut terminal = Terminal::new(backend).unwrap();
    let stdin = io::stdin();
    let mut keys = stdin.keys();

    let items = vec![
        ListItem::new("Option 1"),
        ListItem::new("Option 2"),
        ListItem::new("Option 3"),
    ];

    let mut current_selection = 0;

    loop {
      terminal.draw(|f| {
          let chunks = Layout::default()
              .direction(Direction::Vertical)
              .margin(1)
              .constraints([Constraint::Percentage(100)].as_ref())
              .split(f.size());

          let list = List::new(items.clone())
              .block(Block::default().borders(Borders::ALL))
              .highlight_style(Style::default().add_modifier(Modifier::BOLD).bg(Color::Blue))
              .highlight_symbol(">> ");

          let mut list_state = tui::widgets::ListState::default();
          list_state.select(Some(current_selection));
          f.render_stateful_widget(list, chunks[0], &mut list_state);
      });

      // Update keys handling
      if let Some(Ok(key)) = keys.next() {
          match key {
              Key::Char('q') => break,
              Key::Down => current_selection = (current_selection + 1).min(items.len() - 1),
              Key::Up => if current_selection > 0 { current_selection -= 1 },
              _ => {}
          }
      }
  }
}
