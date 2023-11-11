use crate::helpers::{fetchers::Getter, Challenges};
use egui_commonmark::*;

#[derive(PartialEq, Clone, Copy, serde::Deserialize, serde::Serialize)]
enum FilterOption {
    All,
    UniquePlayers,
    UniqueLanguage,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct ChallengeInfoApp {
    selected_challenge: Challenges,
    #[serde(skip)]
    active_challenge: Challenges,
    #[serde(skip)]
    info_fetcher: Option<Getter<String>>,
    instructions: String,
}

impl Default for ChallengeInfoApp {
    fn default() -> Self {
        Self {
            selected_challenge: Challenges::default(),
            info_fetcher: None,
            active_challenge: Challenges::None,
            instructions: "None".to_string(),
        }
    }
}

impl ChallengeInfoApp {
    fn fetch(&mut self, ctx: &egui::Context) {
        if self.active_challenge == self.selected_challenge {
            return;
        }
        log::debug!("Fetching challenge info");
        self.active_challenge = self.selected_challenge;
        self.info_fetcher = self.selected_challenge.fetcher(Some(ctx));
    }
    fn check_info_promise(&mut self) {
        let getter = &mut self.info_fetcher;

        if let Some(getter) = getter {
            let result = &getter.check_promise();
            self.instructions = result.to_string();
        }
    }
}

impl super::App for ChallengeInfoApp {
    fn name(&self) -> &'static str {
        "📖 Challenge Info"
    }

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        self.fetch(ctx);
        egui::Window::new(self.name())
            .open(open)
            .default_width(800.0)
            .default_height(600.0)
            .vscroll(false)
            .hscroll(false)
            .resizable(true)
            .constrain(true)
            .collapsible(true)
            .show(ctx, |ui| {
                use super::View as _;
                self.ui(ui);
            });
    }
}

impl super::View for ChallengeInfoApp {
    fn ui(&mut self, ui: &mut egui::Ui) {
        self.check_info_promise();
        egui::SidePanel::right("ChallengeInfoSelection")
            .resizable(false)
            .show_inside(ui, |ui| {
                ui.vertical(|ui| {
                    for challenge in Challenges::iter() {
                        ui.radio_value(
                            &mut self.selected_challenge,
                            challenge,
                            format!("{}", challenge),
                        );
                    }
                    ui.separator();
                    if ui.button("Refresh").clicked() {
                        self.active_challenge = Challenges::None;
                    }
                });
            });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let mut cache = CommonMarkCache::default();
                    CommonMarkViewer::new("viewer").show(ui, &mut cache, &self.instructions);
                });
        });
    }
}
