//! The Ask box's state: the model menu, the fast toggle, the job in flight
//! (`io::figure_ai::AskJob`, or a stub channel in tests), its progress.
//!
//! The models come from prefs.lua (`studio = { figure_model, figure_effort,
//! figure_fast_model, figure_fast_effort, figure_fast }`, defaults from
//! figures/eval/REPORT.md); the menu offers `figure_ai::MENU_MODELS`.
use crate::model::StudioPrefs;
pub use ntex_studio_io::figure_ai::AskModel;
use ntex_studio_io::figure_ai::{self, AskEvent, AskJob};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Instant;

/// The menu, in order: prefs' `figure_model` first (selected) when it is not
/// one of the evaluated models, then `MENU_MODELS`, all at `figure_effort`.
/// The index of the default.
pub fn models(prefs: &StudioPrefs) -> (Vec<AskModel>, usize) {
    let effort = prefs.figure_effort.as_str();
    let mut list: Vec<AskModel> = figure_ai::MENU_MODELS.iter().map(|(m, _)| figure_ai::ask_model(m, effort)).collect();
    let want = prefs.figure_model.trim();
    let selected = match list.iter().position(|m| m.model == want) {
        Some(i) => i,
        None => {
            list.insert(0, figure_ai::ask_model(want, effort));
            0
        }
    };
    (list, selected)
}

/// The fast toggle's model: prefs' `figure_fast_model` at `figure_fast_effort`.
pub fn fast_model(prefs: &StudioPrefs) -> AskModel {
    figure_ai::ask_model(&prefs.figure_fast_model, &prefs.figure_fast_effort)
}

/// "Sonnet", "Luna", … (the menu's name when it has one).
pub fn name(m: &AskModel) -> String {
    figure_ai::MENU_MODELS.iter().find(|(id, _)| *id == m.model).map(|(_, n)| n.to_string()).unwrap_or_else(|| figure_ai::display_name(m))
}

pub struct Running {
    /// None: a stub (tests) with nothing to cancel.
    pub job: Option<AskJob>,
    pub rx: Receiver<AskEvent>,
    pub started: Instant,
    pub model: AskModel,
    pub progress: Option<String>,
}

pub struct AskState {
    pub models: Vec<AskModel>,
    pub selected: usize,
    /// The fast toggle (starts as prefs' `figure_fast`).
    pub fast: bool,
    pub fast_model: AskModel,
    pub menu_open: bool,
    pub running: Option<Running>,
    /// The last failure, as a log line under the box.
    pub error: Option<String>,
}

impl Default for AskState {
    fn default() -> Self {
        AskState::from_prefs(&StudioPrefs::default())
    }
}

/// What a pump of the job produced.
pub enum Outcome {
    Nothing,
    Progress,
    Done { source: String, summary: String, seconds: f32 },
    Failed(String),
}

impl AskState {
    pub fn from_prefs(prefs: &StudioPrefs) -> AskState {
        let (models, selected) = models(prefs);
        AskState { models, selected, fast: prefs.figure_fast, fast_model: fast_model(prefs), menu_open: false, running: None, error: None }
    }

    /// The model a Return would use.
    pub fn model(&self) -> AskModel {
        if self.fast { self.fast_model.clone() } else { self.models.get(self.selected).cloned().unwrap_or_else(|| self.fast_model.clone()) }
    }

    pub fn busy(&self) -> bool {
        self.running.is_some()
    }

    pub fn cancel(&mut self) {
        if let Some(mut r) = self.running.take()
            && let Some(job) = r.job.as_mut()
        {
            super::doc::core("AskJob::cancel", || job.cancel());
        }
    }

    pub fn pump(&mut self) -> Outcome {
        let Some(r) = self.running.as_mut() else { return Outcome::Nothing };
        let mut out = Outcome::Nothing;
        loop {
            match r.rx.try_recv() {
                Ok(AskEvent::Progress(p)) => {
                    r.progress = Some(p);
                    out = Outcome::Progress;
                }
                Ok(AskEvent::Done { source, summary, seconds }) => {
                    self.running = None;
                    return Outcome::Done { source, summary, seconds };
                }
                Ok(AskEvent::Failed(e)) => {
                    self.running = None;
                    return Outcome::Failed(e);
                }
                Err(TryRecvError::Empty) => return out,
                Err(TryRecvError::Disconnected) => {
                    self.running = None;
                    return Outcome::Failed("the model stopped without an answer".into());
                }
            }
        }
    }
}
