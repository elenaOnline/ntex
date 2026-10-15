//! Background launch mode: windows open "waiting", never taking focus.
//!
//! `ntex-studio --background` (or `NTEX_STUDIO_BACKGROUND=1`) creates the
//! windows visible but never key, never calls `cx.activate`, and arranges the
//! process so macOS does not bring it to the front. It governs only the
//! request that asked for it (the flag travels in `OpenRequest.background`);
//! a later normal launch / Space n behaves as always (activate and focus).
//!
//! `NTEX_STUDIO_QUIET` (tests) wins over background: nothing is shown at all.
//!
//! The decisions are pure (`Mode`, `plan`) and unit-tested; the AppKit calls
//! (libobjc, no dependency) are the thin `mac` part.

/// How a window-opening request treats focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// As ever: show, make key, activate the app.
    Normal,
    /// Shown, never key, never activates, ordered behind the other windows.
    Background,
    /// Smoke tests: nothing shown, nothing activated.
    Quiet,
}

impl Mode {
    pub fn of(quiet: bool, background: bool) -> Mode {
        if quiet {
            Mode::Quiet
        } else if background {
            Mode::Background
        } else {
            Mode::Normal
        }
    }
}

/// What to do with one window (the `WindowOptions` flags and the follow-ups).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    /// `WindowOptions::focus` (makeKeyAndOrderFront when true).
    pub focus: bool,
    /// `WindowOptions::show` (with focus false: plain orderFront).
    pub show: bool,
    /// call `cx.activate(..)` / `window.activate_window()`.
    pub activate: bool,
    /// afterwards order our windows behind the frontmost app's
    /// (only when our app is not the active one).
    pub order_back: bool,
}

/// `visible`: this window is meant to be seen by this request (false for the
/// studio's own window when only a figure was asked for).
pub fn plan(mode: Mode, visible: bool) -> Plan {
    match mode {
        Mode::Normal => Plan { focus: visible, show: visible, activate: visible, order_back: false },
        Mode::Background => Plan { focus: false, show: visible, activate: false, order_back: visible },
        Mode::Quiet => Plan { focus: false, show: false, activate: false, order_back: false },
    }
}

/// The first real activation (she clicked a window: it became active/key)
/// ends background mode: pure decision, `held` = the Accessory hold is still on.
pub fn should_release(held: bool, window_active: bool) -> bool {
    held && window_active
}

/// Watch this view's window: when it becomes active, release background mode
/// (once; afterwards the app is Regular: Dock icon, menu bar, ⌘-Tab).
pub fn release_on_activation<T: 'static>(window: &mut gpui::Window, cx: &mut gpui::Context<T>) {
    cx.observe_window_activation(window, |_, window, _| {
        if should_release(is_held(), window.is_window_active()) {
            release_background();
        }
    })
    .detach();
}

/// `--background` among the arguments, or the env value (`NTEX_STUDIO_BACKGROUND`)
/// being anything but empty / "0".
pub fn background_requested(args: &[String], env: Option<&str>) -> bool {
    args.iter().any(|a| a == "--background") || env.is_some_and(|v| !v.is_empty() && v != "0")
}

#[cfg(target_os = "macos")]
mod mac {
    use std::ffi::{c_char, c_void};
    use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

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
    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {}

    const REGULAR: isize = 0;
    const ACCESSORY: isize = 1;

    static ORIGINAL: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
    /// While set, a requested Regular policy becomes Accessory.
    static HOLD: AtomicBool = AtomicBool::new(false);

    extern "C" fn set_policy(this: Id, sel: Sel, requested: isize) -> bool {
        let orig = ORIGINAL.load(Ordering::SeqCst);
        if orig.is_null() {
            return false;
        }
        let policy = if HOLD.load(Ordering::SeqCst) && requested == REGULAR { ACCESSORY } else { requested };
        // SAFETY: the original IMP of -[NSApplication setActivationPolicy:]
        let f: extern "C" fn(Id, Sel, isize) -> bool = unsafe { std::mem::transmute(orig) };
        f(this, sel, policy)
    }

    /// Before the NSApplication exists: GPUI sets Regular when launching
    /// finishes, which makes macOS activate a freshly started process. Make
    /// that Accessory (no Dock icon, never auto-activated) until a normal
    /// request releases it. Returns false when it could not be arranged.
    pub fn hold_background() -> bool {
        unsafe {
            let cls = objc_getClass(c"NSApplication".as_ptr());
            if cls.is_null() {
                return false;
            }
            let m = class_getInstanceMethod(cls, sel_registerName(c"setActivationPolicy:".as_ptr()));
            if m.is_null() {
                return false;
            }
            HOLD.store(true, Ordering::SeqCst);
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

    pub fn is_held() -> bool {
        HOLD.load(Ordering::SeqCst)
    }

    /// A normal request arrives at a studio that was launched in the
    /// background: become a Regular app (Dock icon, menu bar) again.
    pub fn release_background() {
        if HOLD.swap(false, Ordering::SeqCst) {
            unsafe {
                let send: extern "C" fn(Id, Sel, isize) -> bool = std::mem::transmute(objc_msgSend as *const ());
                send(ns_app(), sel_registerName(c"setActivationPolicy:".as_ptr()), REGULAR);
            }
        }
    }

    /// `[NSApp isActive]`
    fn is_active() -> bool {
        unsafe {
            let send: extern "C" fn(Id, Sel) -> bool = std::mem::transmute(objc_msgSend as *const ());
            send(ns_app(), sel_registerName(c"isActive".as_ptr()))
        }
    }

    /// If our app is not active: `[w orderBack:nil]` for each visible window
    /// that is not key, so they wait behind whatever you are working in.
    pub fn order_back_if_inactive() {
        if is_active() {
            return;
        }
        unsafe {
            let msg0: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(objc_msgSend as *const ());
            let msg_bool: extern "C" fn(Id, Sel) -> bool = std::mem::transmute(objc_msgSend as *const ());
            let msg_idx: extern "C" fn(Id, Sel, usize) -> Id = std::mem::transmute(objc_msgSend as *const ());
            let msg_len: extern "C" fn(Id, Sel) -> usize = std::mem::transmute(objc_msgSend as *const ());
            let msg_obj: extern "C" fn(Id, Sel, Id) = std::mem::transmute(objc_msgSend as *const ());
            let windows = msg0(ns_app(), sel_registerName(c"windows".as_ptr()));
            let n = msg_len(windows, sel_registerName(c"count".as_ptr()));
            for i in 0..n {
                let w = msg_idx(windows, sel_registerName(c"objectAtIndex:".as_ptr()), i);
                if msg_bool(w, sel_registerName(c"isVisible".as_ptr())) && !msg_bool(w, sel_registerName(c"isKeyWindow".as_ptr())) {
                    msg_obj(w, sel_registerName(c"orderBack:".as_ptr()), std::ptr::null_mut());
                }
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub use mac::{hold_background, is_held, order_back_if_inactive, release_background};

#[cfg(not(target_os = "macos"))]
pub fn hold_background() -> bool {
    true
}
#[cfg(not(target_os = "macos"))]
pub fn release_background() {}
#[cfg(not(target_os = "macos"))]
pub fn is_held() -> bool {
    false
}
#[cfg(not(target_os = "macos"))]
pub fn order_back_if_inactive() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes() {
        assert_eq!(Mode::of(false, false), Mode::Normal);
        assert_eq!(Mode::of(false, true), Mode::Background);
        assert_eq!(Mode::of(true, true), Mode::Quiet, "quiet wins");
        assert_eq!(Mode::of(true, false), Mode::Quiet);
    }

    #[test]
    fn normal_focuses_and_activates() {
        assert_eq!(plan(Mode::Normal, true), Plan { focus: true, show: true, activate: true, order_back: false });
        let hidden = plan(Mode::Normal, false);
        assert!(!hidden.focus && !hidden.show && !hidden.activate);
    }

    #[test]
    fn background_shows_without_focus_or_activation() {
        let p = plan(Mode::Background, true);
        assert!(p.show && !p.focus && !p.activate && p.order_back);
        let h = plan(Mode::Background, false);
        assert!(!h.show && !h.focus && !h.activate && !h.order_back);
    }

    #[test]
    fn quiet_shows_nothing() {
        for v in [true, false] {
            assert_eq!(plan(Mode::Quiet, v), Plan { focus: false, show: false, activate: false, order_back: false });
        }
    }

    #[test]
    fn release_only_on_first_real_activation() {
        assert!(should_release(true, true));
        assert!(!should_release(true, false), "deactivation / creation does not release");
        assert!(!should_release(false, true), "already released (or never held)");
        assert!(!should_release(false, false));
    }

    #[test]
    fn flag_and_env() {
        let a = |s: &[&str]| s.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert!(background_requested(&a(&["--background"]), None));
        assert!(background_requested(&a(&["--figure", "x.fig", "--background"]), None));
        assert!(background_requested(&a(&[]), Some("1")));
        assert!(!background_requested(&a(&[]), Some("0")));
        assert!(!background_requested(&a(&[]), Some("")));
        assert!(!background_requested(&a(&["--page", "inbox"]), None));
    }
}
