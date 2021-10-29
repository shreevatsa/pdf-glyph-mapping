//! Parses a PDF file, and dumps the following:
//!
//! 1.  For each font, its ToUnicode mapping (if present), with the font's /BaseFont name.
//! 2.  For each font, the operands of each text-showing (`Tj` etc) operation that uses that font.

use anyhow::Result;
use clap::Clap;
use env_logger::WriteStyle;
use log::LevelFilter;
use std::collections::HashMap;
use std::io::Write;

mod pdf_visit;
mod text_state;

#[derive(Clone)]
pub enum Phase {
    Phase1Dump,
    Phase2Fix,
}
impl std::str::FromStr for Phase {
    type Err = std::num::ParseIntError;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if s == "phase2" {
            Ok(Phase::Phase2Fix)
        } else {
            Ok(Phase::Phase1Dump)
        }
    }
}

/// Parse a PDF file either to dump text operations (Tj etc) in it,
/// or to "fix" all text by surrounding them with /ActualText.
fn main() -> Result<()> {
    // env_logger::init();
    // pretty_env_logger::init();
    // flexi_logger::Logger::try_with_str("info")?
    //     .format(|w, _now, record| {
    //         write!(
    //             w,
    //             // "[{}] {} [{}] {}:{}: {}",
    //             "{} {}:{}: {}",
    //             // now.now().format("%Y-%m-%d %H:%M:%S%.6f %:z"),
    //             record.level(),
    //             // record.module_path().unwrap_or("<unnamed>"),
    //             record.file().unwrap_or("<unnamed>"),
    //             record.line().unwrap_or(0),
    //             &record.args()
    //         )
    //     })
    //     .start()?;
    env_logger::Builder::from_default_env()
        // TODO: Figure out how to write a custom format function that doesn't disable the default colors.
        .format(|buf, record| {
            writeln!(
                buf,
                "{} {}:{}: {}",
                record.level(),
                record.file().unwrap_or("<unnamed>"),
                record.line().unwrap_or(0),
                record.args()
            )
        })
        //.filter(None, LevelFilter::Info)
        .init();

    #[derive(Clap)]
    struct Opts {
        // #[clap(parse(from_os_str), value_hint = clap::ValueHint::AnyPath)]
        input_pdf_file: std::path::PathBuf,
        /// Whether to dump (phase 1) or fix (phase 2).
        #[clap(long)]
        phase: Phase,
        /// Operate on just a single page (should this take ranges?)
        #[clap(long)]
        page: Option<u32>,
        /// verbose output
        #[clap(long)]
        debug: bool,
        /// The directory for either the maps to dump (Phase 1), or maps to read (Phase 2).
        maps_dir: std::path::PathBuf,
        output_pdf_file: Option<std::path::PathBuf>,
        /// Run the profiler. May cause SIGSEGV: https://github.com/tikv/pprof-rs/issues/76
        #[clap(long)]
        profile: bool,
    }

    let opts = Opts::parse();
    let filename = opts.input_pdf_file;

    let start = std::time::Instant::now();
    println!("Loading {:?}", filename);
    let mut document = lopdf::Document::load(&filename).expect("could not load PDF file");
    let end = std::time::Instant::now();
    println!("Loaded {:?} in {:?}", &filename, end.duration_since(start));

    if let Phase::Phase1Dump = opts.phase {
        text_state::dump_unicode_mappings(&document, opts.maps_dir.clone()).unwrap_or(());
    }

    let guard = match opts.profile {
        true => pprof::ProfilerGuard::new(100).ok(),
        false => None,
    };

    {
        let mut visitor = text_state::MyOpVisitor {
            text_state: text_state::TextState {
                current_font: pdf_visit::Font {
                    font_descriptor_id: None,
                    base_font_name: None,
                    encoding: None,
                    subtype: None,
                    to_unicode: None,
                    font_descriptor: None,
                },
                current_tm_c: 0.0,
            },
            maps_dir: opts.maps_dir,
            files: text_state::TjFiles {
                file: HashMap::new(),
            },
            font_glyph_mappings: HashMap::new(),
            phase: opts.phase.clone(),
        };
        pdf_visit::visit_page_content_stream_ops(
            &mut document,
            &mut visitor,
            opts.page,
            opts.debug,
        )
        .unwrap();
        if let Phase::Phase2Fix = opts.phase {
            visitor.dump_font_glyph_mappings();
            if let Some(output_pdf_filename) = opts.output_pdf_file {
                println!("Saving result to PDF file: {:?}", output_pdf_filename);
                ok!(document.save(output_pdf_filename));
            } else {
                todo!()
            }
        }
    }

    if let Some(guard) = guard {
        if let Ok(report) = guard.report().build() {
            let file = std::fs::File::create("flamegraph.svg")?;
            report.flamegraph(file)?;
        }
    }
    Ok(())
}
