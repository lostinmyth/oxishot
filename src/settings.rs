//! The settings dialog: shows the config file, edits a draft of it, and
//! writes the draft back on Save.

use std::{cell::RefCell, rc::Rc};

use slint::{ComponentHandle, Model, ModelRc, VecModel};

use crate::configuration::config::{self, BINDABLE, Combo, Config};
use crate::{Binding, MainWindow, Settings, Shortcut};

pub struct SettingsUi {
    /// What's in use: the file as last loaded or saved.
    pub config: Rc<RefCell<Config>>,
    /// The dialog's shortcuts, until saved.
    draft: RefCell<Vec<Vec<Combo>>>,
    rows: Rc<VecModel<Binding>>,
}

impl SettingsUi {
    pub fn install(win: &MainWindow, config: Config) -> Rc<SettingsUi> {
        let ui = Rc::new(SettingsUi {
            draft: RefCell::new(config.keys.clone()),
            config: Rc::new(RefCell::new(config)),
            rows: Rc::new(VecModel::default()),
        });
        let s = win.global::<Settings>();
        s.set_bindings(ModelRc::from(ui.rows.clone()));
        if let Some(p) = Config::path() {
            s.set_path(p.display().to_string().into());
        }

        s.on_shortcut({
            let config = ui.config.clone();
            move |text, ctrl, alt, shift| {
                let action = Combo::from_event(&text, ctrl, alt, shift)
                    .and_then(|c| config.borrow().action_for(&c));
                match action {
                    Some(action) => Shortcut { ok: true, action },
                    None => Shortcut::default(),
                }
            }
        });
        s.on_label({
            let config = ui.config.clone();
            move |action, _version| config.borrow().label(action).into()
        });
        s.on_capture({
            let ui = Rc::downgrade(&ui);
            move |i, text, ctrl, alt, shift| {
                let (Some(ui), Some(combo)) = (ui.upgrade(), Combo::from_event(&text, ctrl, alt, shift)) else {
                    return false;
                };
                ui.set_binding(i as usize, vec![combo]);
                true
            }
        });
        s.on_clear_binding({
            let ui = Rc::downgrade(&ui);
            move |i| {
                if let Some(ui) = ui.upgrade() {
                    ui.set_binding(i as usize, Vec::new());
                }
            }
        });
        s.on_apply({
            let (ui, win) = (Rc::downgrade(&ui), win.as_weak());
            move || {
                let (Some(ui), Some(win)) = (ui.upgrade(), win.upgrade()) else { return };
                let s = win.global::<Settings>();
                let next = Config {
                    directory: s.get_directory().trim().to_string(),
                    skip_dialog: s.get_skip_dialog(),
                    keys: ui.draft.borrow().clone(),
                };
                match next.save() {
                    Ok(_) => {
                        *ui.config.borrow_mut() = next;
                        s.set_version(s.get_version() + 1);
                        s.set_error("".into());
                        s.set_shown(false);
                    }
                    Err(e) => s.set_error(e.into()),
                }
            }
        });
        s.on_browse({
            let win = win.as_weak();
            move || {
                let Some(w) = win.upgrade() else { return };
                let mut dialog = rfd::AsyncFileDialog::new().set_parent(&w.window().window_handle());
                let current = w.global::<Settings>().get_directory();
                if let Some(dir) = config::expand_dir(&current).filter(|d| d.is_dir()) {
                    dialog = dialog.set_directory(dir);
                }
                let win = win.clone();
                let _ = slint::spawn_local(async move {
                    let Some(dir) = dialog.pick_folder().await else { return };
                    if let Some(w) = win.upgrade() {
                        w.global::<Settings>().set_directory(dir.path().display().to_string().into());
                    }
                });
            }
        });
        s.on_edit_file({
            let win = win.as_weak();
            move || {
                let Some(w) = win.upgrade() else { return };
                let s = w.global::<Settings>();
                match Config::ensure_file().and_then(|p| open_in_editor(&p)) {
                    // the dialog would show stale values; reopening reloads the file
                    Ok(()) => s.set_shown(false),
                    Err(e) => s.set_error(e.into()),
                }
            }
        });
        ui
    }

    /// Reloads the config file, puts it to use, and opens the dialog on it.
    pub fn show(&self, win: &MainWindow) {
        let (config, errors) = Config::load();
        let s = win.global::<Settings>();
        s.set_directory(config.directory.clone().into());
        s.set_skip_dialog(config.skip_dialog);
        s.set_error(errors.join("\n").into());
        *self.draft.borrow_mut() = config.keys.clone();
        *self.config.borrow_mut() = config;
        s.set_version(s.get_version() + 1);
        self.refresh_rows();
        s.set_shown(true);
    }

    fn set_binding(&self, i: usize, combos: Vec<Combo>) {
        if let Some(slot) = self.draft.borrow_mut().get_mut(i) {
            *slot = combos;
        }
        self.refresh_rows();
    }

    fn refresh_rows(&self) {
        let draft = self.draft.borrow();
        let rows: Vec<Binding> = BINDABLE
            .iter()
            .zip(draft.iter())
            .enumerate()
            .map(|(i, (&(_, _, name), combos))| Binding {
                name: name.into(),
                keys: config::join(combos).into(),
                conflict: combos
                    .iter()
                    .any(|c| draft.iter().enumerate().any(|(j, other)| j != i && other.contains(c))),
            })
            .collect();
        if self.rows.row_count() == rows.len() {
            for (i, r) in rows.into_iter().enumerate() {
                self.rows.set_row_data(i, r);
            }
        } else {
            self.rows.set_vec(rows);
        }
    }
}

/// Opens `path` with the desktop's handler for it.
fn open_in_editor(path: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut cmd = std::process::Command::new("open");
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    };
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut cmd = std::process::Command::new("xdg-open");
    cmd.arg(path)
        .spawn()
        .map(drop)
        .map_err(|e| format!("could not open {}: {e}", path.display()))
}
