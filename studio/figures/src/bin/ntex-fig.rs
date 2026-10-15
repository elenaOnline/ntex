//! ntex-fig [-v] build|check|tikz [--portable]|fmt|new <file.fig>   (FIG-CORE-B)
//!
//! new     writes the starting file (figures/template.fig: two comment
//!         lines naming the figure and the reference) to <file.fig>, only if
//!         it doesn't exist yet
//! build   writes <file>.pdf next to it (tectonic, her theme); prints the
//!         diagnostics; exit 1 on a TeX error (the old PDF is kept). An
//!         empty figure gets a placeholder PDF ("empty figure — NAME"), exit 0
//! check   prints the diagnostics as `file:line:col: severity: message`;
//!         exit 1 if any is an error
//! tikz    prints the tikzpicture (`--portable`: with colours and styles
//!         inlined, for a .tex without ntex-figure.sty)
//! fmt     rewrites the file in canonical form
//!
//! -v      also prints the notes ("`A` is new here, so it became a node."),
//!         which come on almost every line; by default only errors and
//!         warnings are printed
//!
//! The theme is found in $NTEX_STUDIO_CONFIG/templates/tex, else
//! ~/.config/texnvim/templates/tex.
use ntex_figures::model::*;
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "usage: ntex-fig [-v] build|check|tikz [--portable]|fmt|new <file.fig>";

fn severity(s: Severity) -> &'static str {
    match s {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Note => "note",
    }
}

/// Errors and warnings; the notes too with `-v`.
fn print_diags(path: &Path, diags: &[Diagnostic], verbose: bool) {
    for d in diags.iter().filter(|d| verbose || d.severity != Severity::Note) {
        eprintln!("{}:{}:{}: {}: {}", path.display(), d.span.line, d.span.col.max(1), severity(d.severity), d.message);
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let portable = args.iter().any(|a| a == "--portable");
    let verbose = args.iter().any(|a| a == "-v" || a == "--verbose");
    let rest: Vec<&String> = args.iter().filter(|a| !a.starts_with("--") && a.as_str() != "-v").collect();
    if args.iter().any(|a| a == "--help" || a == "-h") || rest.len() != 2 {
        eprintln!("{USAGE}");
        return if rest.is_empty() && args.iter().any(|a| a == "--help" || a == "-h") { ExitCode::SUCCESS } else { ExitCode::from(2) };
    }
    let (cmd, path) = (rest[0].as_str(), Path::new(rest[1].as_str()));
    if !["build", "check", "tikz", "fmt", "new"].contains(&cmd) {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    if cmd == "new" {
        return match ntex_figures::build::new_figure(path) {
            Ok(true) => {
                println!("{}", path.display());
                ExitCode::SUCCESS
            }
            Ok(false) => {
                eprintln!("{}: exists, left as it is", path.display());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("ntex-fig: {e}");
                ExitCode::from(1)
            }
        };
    }
    let src = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("ntex-fig: can't read {}: {e}", path.display());
            return ExitCode::from(1);
        }
    };
    match cmd {
        "build" => {
            let (fig, mut diags) = ntex_figures::parse::parse(&src);
            diags.extend(ntex_figures::build::figure_diagnostics(&fig));
            print_diags(path, &diags, verbose);
            let out = path.with_extension("pdf");
            match ntex_figures::build::build_figure(&fig, &out, &ntex_figures::build::templates_dir()) {
                Ok(b) => {
                    // (the diagnostics are printed above; the empty note isn't one)
                    for w in b.warnings.iter().filter(|w| w.as_str() == ntex_figures::build::EMPTY_NOTE) {
                        eprintln!("{}: {w}", path.display());
                    }
                    println!("{}", b.pdf.display());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("{}: {e}", path.display());
                    ExitCode::from(1)
                }
            }
        }
        "check" => {
            let (fig, mut diags) = ntex_figures::parse::parse(&src);
            diags.extend(ntex_figures::build::figure_diagnostics(&fig));
            print_diags(path, &diags, verbose);
            if diags.iter().any(|d| d.severity == Severity::Error) { ExitCode::from(1) } else { ExitCode::SUCCESS }
        }
        "tikz" => {
            let (fig, diags) = ntex_figures::parse::parse(&src);
            print_diags(path, &diags, verbose);
            let scene = ntex_figures::layout::layout(&fig);
            if portable {
                print!("{}", ntex_figures::tikz::portable(&fig, &scene));
            } else {
                print!("{}", ntex_figures::tikz::picture(&fig, &scene));
            }
            ExitCode::SUCCESS
        }
        "fmt" => {
            let (fig, diags) = ntex_figures::parse::parse(&src);
            print_diags(path, &diags, verbose);
            let out = ntex_figures::format::format(&fig);
            if out != src {
                let tmp = path.with_extension("fig.part");
                if let Err(e) = std::fs::write(&tmp, &out).and_then(|_| std::fs::rename(&tmp, path)) {
                    eprintln!("ntex-fig: can't write {}: {e}", path.display());
                    let _ = std::fs::remove_file(&tmp);
                    return ExitCode::from(1);
                }
            }
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}
