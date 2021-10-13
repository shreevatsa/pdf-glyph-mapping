//! Parses a PDF file, and dumps the following:
//!
//! 1.  For each font, its ToUnicode mapping (if present), with the font's /BaseFont name.
//! 2.  For each font, the operands of each text-showing (`Tj` etc) operation that uses that font.

use anyhow::Result;
use clap::Clap;
use std::collections::HashMap;

mod pdf_visit;
mod text_state;

#[derive(Clone)]
pub enum Phase {
    Phase1Dump,
    Phase2Fix,
}

/// Parse a PDF file either to dump text operations (Tj etc) in it,
/// or to "fix" all text by surrounding them with /ActualText.
fn main() -> Result<()> {
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
    }

    let opts = Opts::parse();
    let filename = opts.input_pdf_file;

    let start = std::time::Instant::now();
    println!("Loading {:?}", filename);
    let mut document = lopdf::Document::load(&filename).expect("could not load PDF file");
    let end = std::time::Instant::now();
    println!("Loaded {:?} in {:?}", &filename, end.duration_since(start));

    if let Phase::Phase1Dump = opts.phase {
        text_state::dump_unicode_mappings(&mut document, opts.maps_dir.clone()).unwrap();
    }

    // let guard = pprof::ProfilerGuard::new(100)?;

    {
        let mut visitor = text_state::MyOpVisitor {
            text_state: text_state::TextState {
                current_font: ("".to_string(), (0, 0)),
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
                println!("Creating file: {:?}", output_pdf_filename);
                document.save(output_pdf_filename)?;
            } else {
                todo!()
            }
        }
    }

    // if let Ok(report) = guard.report().build() {
    //     let file = File::create("flamegraph.svg")?;
    //     report.flamegraph(file)?;
    // };
    Ok(())
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
