//! `ntex-studio [--page snippet|block|inbox] [--output-file PATH] [--context math|text] [--ft tex|markdown] [--figure PATH]`
//!
//! `--figure PATH` opens (or focuses) the figure window for that `.fig`
//! (FIGURES.md "The figure window"); a missing file starts from the template.
//!
//! One studio at a time: a second invocation (ntex's Space n) hands its
//! request to the running one over the socket and exits; the first becomes
//! the studio. ⌘W / Esc hide it (the next Space n is instant); ⌘Q quits and
//! stops the bridge.
//!
//! `NTEX_STUDIO_QUIET=1` is the smoke-test mode: it runs the real binary
//! without ever showing anything. The activation policy is Prohibited (no
//! Dock icon, no activation, no menu bar), the window is created hidden and
//! unfocused, nothing activates the app, "ready" and the first-frame timing go
//! to stderr, and `NTEX_STUDIO_QUIT_AFTER_MS` makes it quit by itself.
use ntex_studio::io::instance::{self, Instance};
use ntex_studio::model::Paths;
use std::time::{Duration, Instant};

fn main() {
    let started = Instant::now();
    let _ = ntex_studio::ui::app::STARTED.set(started);
    let quiet = std::env::var("NTEX_STUDIO_QUIET").is_ok_and(|v| !v.is_empty() && v != "0");
    if quiet {
        // before anything else: every activation policy this process sets
        // (GPUI sets Regular when launching finishes) becomes Prohibited
        if !quiet::prohibit_activation() {
            eprintln!("ntex-studio: quiet mode could not make activation impossible; not starting");
            std::process::exit(2);
        }
    }
    let quit_after = std::env::var("NTEX_STUDIO_QUIT_AFTER_MS").ok().and_then(|v| v.trim().parse::<u64>().ok());

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut first = instance::parse_args(&args);
    // FIG-UI: `--figure PATH` (or `--figure=PATH`): that figure's window
    first.figure = figure_arg(&args);
    let has_figure = first.figure.is_some();
    let has_page = first.page.is_some() || first.output.is_some();
    let paths = Paths::from_env();
    let requests = match instance::claim(&paths, &first) {
        Ok(Instance::Forwarded) => {
            if quiet {
                eprintln!("ntex-studio: forwarded to the running studio");
            }
            return;
        }
        Ok(Instance::Primary(rx)) => Some(rx),
        Err(e) => {
            // no socket: still be the studio, just not reachable by Space n
            eprintln!("ntex-studio: single-instance socket unavailable ({e}); running anyway");
            None
        }
    };
    gpui_platform::application().run(move |cx| {
        if quiet {
            quiet::prohibit_now();
            // fail closed: never open a window unless the app is Prohibited
            if quiet::policy() != 2 {
                eprintln!("ntex-studio: quiet mode: activation policy is {}, not Prohibited; quitting", quiet::policy());
                std::process::exit(3);
            }
        }
        let t_init = Instant::now();
        ntex_studio::ui::app::init(cx);
        if quiet {
            eprintln!("ntex-studio: run callback at {:.1} ms; init (fonts, keys) {:.1} ms", started.elapsed().as_secs_f64() * 1000.0 - t_init.elapsed().as_secs_f64() * 1000.0, t_init.elapsed().as_secs_f64() * 1000.0);
        }
        let t_window = Instant::now();
        if ntex_studio::ui::app::open_window_with(cx, first, requests, quiet).is_none() {
            eprintln!("ntex-studio: could not open the window");
            cx.quit();
            return;
        }
        let figure_only = has_figure && !has_page;
        if quiet {
            // open_window lays out and paints the first frame before it returns
            eprintln!(
                "ntex-studio: ready · first frame {:.1} ms after launch (window + first layout/paint {:.1} ms) · activation policy {}",
                started.elapsed().as_secs_f64() * 1000.0,
                t_window.elapsed().as_secs_f64() * 1000.0,
                quiet::policy(),
            );
        } else if !figure_only {
            // (a figure window activates itself when it opens)
            cx.activate(true);
        }
        if let Some(ms) = quit_after {
            cx.spawn(async move |cx| {
                cx.background_executor().timer(Duration::from_millis(ms)).await;
                cx.update(|cx| {
                    if quiet {
                        eprintln!("ntex-studio: quitting after {ms} ms ({} figure window(s))", ntex_studio::ui::figure::open_count(cx));
                    }
                    cx.quit();
                });
            })
            .detach();
        }
    });
}

/// `--figure PATH` / `--figure=PATH`, made absolute against the working
/// directory (the running studio may have another one).
fn figure_arg(args: &[String]) -> Option<String> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let v = if a == "--figure" {
            it.next().cloned()
        } else {
            a.strip_prefix("--figure=").map(str::to_string)
        };
        if let Some(v) = v.filter(|v| !v.is_empty()) {
            let p = std::path::PathBuf::from(&v);
            let p = if p.is_absolute() { p } else { std::env::current_dir().unwrap_or_default().join(p) };
            return Some(ntex_studio::ui::figure::normalize(&p).to_string_lossy().into_owned());
        }
    }
    None
}

/// The Objective-C runtime calls for the quiet mode (no dependency: libobjc).
#[cfg(target_os = "macos")]
mod quiet {
    use std::ffi::{c_char, c_void};
    use std::sync::atomic::{AtomicPtr, Ordering};

    type Id = *mut c_void;
    type Sel = *mut c_void;
    type Class = *mut c_void;
    type Method = *mut c_void;
    type Imp = *mut c_void;

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Class;
        fn sel_registerName(name: *const c_char) -> Sel;
        fn class_getInstanceMethod(cls: Class, sel: Sel) -> Method;
        fn method_setImplementation(m: Method, imp: Imp) -> Imp;
        fn objc_msgSend();
    }
    // AppKit is linked by GPUI; name it so the class exists at startup
    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {}

    /// NSApplicationActivationPolicyProhibited
    const PROHIBITED: isize = 2;

    static ORIGINAL: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

    extern "C" fn set_policy(this: Id, sel: Sel, _requested: isize) -> bool {
        let orig = ORIGINAL.load(Ordering::SeqCst);
        if orig.is_null() {
            return false;
        }
        // SAFETY: the original IMP of -[NSApplication setActivationPolicy:]
        let f: extern "C" fn(Id, Sel, isize) -> bool = unsafe { std::mem::transmute(orig) };
        f(this, sel, PROHIBITED)
    }

    /// Replace -[NSApplication setActivationPolicy:] so whatever is asked for,
    /// Prohibited is what's set. Call before the NSApplication exists.
    pub fn prohibit_activation() -> bool {
        unsafe {
            let cls = objc_getClass(c"NSApplication".as_ptr());
            let sel = sel_registerName(c"setActivationPolicy:".as_ptr());
            if cls.is_null() {
                eprintln!("ntex-studio: quiet: NSApplication not found");
                return false;
            }
            let m = class_getInstanceMethod(cls, sel);
            if m.is_null() {
                eprintln!("ntex-studio: quiet: setActivationPolicy: not found");
                return false;
            }
            let old = method_setImplementation(m, set_policy as *const () as Imp);
            ORIGINAL.store(old, Ordering::SeqCst);
            !old.is_null()
        }
    }

    fn ns_app() -> Id {
        unsafe {
            let cls = objc_getClass(c"NSApplication".as_ptr());
            let send: extern "C" fn(Class, Sel) -> Id = std::mem::transmute(objc_msgSend as *const ());
            send(cls, sel_registerName(c"sharedApplication".as_ptr()))
        }
    }

    /// Once NSApp exists (inside `run`): set Prohibited explicitly too.
    pub fn prohibit_now() {
        unsafe {
            let send: extern "C" fn(Id, Sel, isize) -> bool = std::mem::transmute(objc_msgSend as *const ());
            send(ns_app(), sel_registerName(c"setActivationPolicy:".as_ptr()), PROHIBITED);
        }
    }

    /// The current policy, for the log: 0 regular, 1 accessory, 2 prohibited.
    pub fn policy() -> isize {
        unsafe {
            let send: extern "C" fn(Id, Sel) -> isize = std::mem::transmute(objc_msgSend as *const ());
            send(ns_app(), sel_registerName(c"activationPolicy".as_ptr()))
        }
    }
}
