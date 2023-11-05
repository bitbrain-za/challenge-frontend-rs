use crate::helpers::{refresh, Challenges};
use gloo_net::http;
use poll_promise::Promise;
use scoreboard_db::Builder as FilterBuilder;
use scoreboard_db::Filter as ScoreBoardFilter;
use scoreboard_db::{NiceTime, Score, ScoreBoard, SortColumn};
use std::str::FromStr;
use web_sys::RequestCredentials;

#[derive(PartialEq, Clone, Copy, serde::Deserialize, serde::Serialize)]
enum FilterOption {
    All,
    UniquePlayers,
    UniqueLanguage,
}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
enum FetchResponse {
    Success(Vec<Score>),
    Failure(String),
    FailAuth,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct ScoreBoardApp {
    challenge: Challenges,
    filter: FilterOption,
    sort_column: String,

    active_challenge: Challenges,
    active_filter: FilterOption,
    active_sort_column: String,

    scores: Option<Vec<Score>>,

    #[serde(skip)]
    promise: Option<Promise<FetchResponse>>,
    #[serde(skip)]
    token_refresh_promise: Option<Promise<Result<refresh::RefreshResponse, String>>>,
    #[serde(skip)]
    refresh_token: bool,
    #[serde(skip)]
    url: String,
    #[serde(skip)]
    refresh: bool,
}

impl Default for ScoreBoardApp {
    fn default() -> Self {
        Self {
            challenge: Challenges::default(),
            filter: FilterOption::All,
            sort_column: "time".to_string(),
            promise: None,
            token_refresh_promise: None,
            refresh_token: false,
            url: option_env!("BACKEND_URL")
                .unwrap_or("http://123.4.5.6:3000/")
                .to_string(),
            refresh: true,

            active_challenge: Challenges::default(),
            active_filter: FilterOption::All,
            active_sort_column: "time".to_string(),
            scores: None,
        }
    }
}

impl ScoreBoardApp {
    fn fetch(&mut self, ctx: &egui::Context) {
        if !self.refresh {
            return;
        }
        self.refresh = false;
        self.scores = None;

        let url = format!("{}api/game/scores/{}", self.url, self.challenge);
        let ctx = ctx.clone();

        let promise = poll_promise::Promise::spawn_local(async move {
            let response = http::Request::get(&url).credentials(RequestCredentials::Include);
            let response = response.send().await.unwrap();
            let text = response.text().await;
            let text = text.map(|text| text.to_owned());

            let result = match response.status() {
                200 => {
                    let scores: Vec<Score> = serde_json::from_str(text.as_ref().unwrap()).unwrap();
                    FetchResponse::Success(scores)
                }
                401 => {
                    let text = match text {
                        Ok(text) => text,
                        Err(e) => e.to_string(),
                    };
                    log::warn!("Auth Error: {:?}", text);
                    FetchResponse::FailAuth
                }

                _ => {
                    log::error!("Response: {:?}", text);
                    FetchResponse::Failure(text.unwrap())
                }
            };
            ctx.request_repaint(); // wake up UI thread
            result
        });
        self.promise = Some(promise);
    }

    fn check_for_reload(&mut self) {
        if self.active_challenge != self.challenge
            || self.active_filter != self.filter
            || self.active_sort_column != self.sort_column
        {
            self.active_challenge = self.challenge;
            self.active_filter = self.filter;
            self.active_sort_column = self.sort_column.clone();
            self.refresh = true;
        }
    }

    fn check_fetch_promise(&mut self) -> Option<FetchResponse> {
        if let Some(promise) = &self.promise {
            if let Some(result) = promise.ready() {
                if let FetchResponse::FailAuth = result {
                    self.refresh_token = true;
                    self.token_refresh_promise = Some(refresh::submit_refresh(&self.url));
                }
                let result = Some(result.clone());
                self.promise = None;
                return result;
            }
        }
        None
    }

    fn check_refresh_promise(&mut self) {
        if let Some(promise) = &self.token_refresh_promise {
            if let Some(result) = promise.ready() {
                if let Ok(result) = result {
                    if "success" == result.status {
                        log::info!("Token refreshed");
                        self.refresh = true;
                    } else {
                        log::error!("Failed to refresh token: {:?}", result);
                    }
                }
                self.refresh_token = false;
                self.token_refresh_promise = None;
            }
        }
    }
}

impl super::App for ScoreBoardApp {
    fn name(&self) -> &'static str {
        "☰ Score Board"
    }

    fn show(&mut self, ctx: &egui::Context, open: &mut bool) {
        self.check_for_reload();
        self.fetch(ctx);
        egui::Window::new(self.name())
            .open(open)
            .default_width(400.0)
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

impl super::View for ScoreBoardApp {
    fn ui(&mut self, ui: &mut egui::Ui) {
        egui::SidePanel::right("Options")
            .resizable(false)
            .show_inside(ui, |ui| {
                ui.vertical(|ui| {
                    egui::ComboBox::from_label("Challenge")
                        .selected_text(format!("{}", self.challenge))
                        .show_ui(ui, |ui| {
                            ui.style_mut().wrap = Some(false);
                            ui.set_min_width(60.0);

                            for challenge in Challenges::iter() {
                                ui.selectable_value(
                                    &mut self.challenge,
                                    challenge,
                                    format!("{}", challenge),
                                );
                            }
                        });

                    ui.separator();
                    ui.label("Filter:");
                    ui.radio_value(&mut self.filter, FilterOption::All, "All");
                    ui.radio_value(
                        &mut self.filter,
                        FilterOption::UniquePlayers,
                        "Unique Players",
                    );
                    ui.radio_value(
                        &mut self.filter,
                        FilterOption::UniqueLanguage,
                        "Unique Langauges",
                    );
                    ui.separator();
                    if ui.button("Refresh").clicked() {
                        self.refresh = true;
                    }
                });
            });
        egui::CentralPanel::default().show_inside(ui, |ui| {
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    self.table_ui(ui);
                });
        });
    }
}

impl ScoreBoardApp {
    fn table_ui(&mut self, ui: &mut egui::Ui) {
        use egui_extras::{Column, TableBuilder};

        if let Some(result) = self.check_fetch_promise() {
            match result {
                FetchResponse::Success(s) => {
                    self.scores = Some(s);
                }
                FetchResponse::Failure(text) => {
                    ui.label(text);
                }
                FetchResponse::FailAuth => {
                    ui.label("Failed to authenticate, refreshing token");
                }
            }
        }
        self.check_refresh_promise();

        let text_height = egui::TextStyle::Body.resolve(ui.style()).size;
        let table = TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::auto())
            .column(Column::auto())
            .column(Column::initial(100.0).range(40.0..=300.0))
            .column(Column::initial(100.0).at_least(40.0).clip(true))
            .column(Column::remainder())
            .min_scrolled_height(0.0);

        table
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.strong("#");
                });
                header.col(|ui| {
                    ui.strong("Time");
                });
                header.col(|ui| {
                    ui.strong("Name");
                });
                header.col(|ui| {
                    ui.strong("Language");
                });
                header.col(|ui| {
                    ui.strong("Binary");
                });
            })
            .body(|mut body| {
                if let Some(scores) = &self.scores {
                    let mut filters = FilterBuilder::new();
                    match self.filter {
                        FilterOption::All => {}
                        FilterOption::UniquePlayers => {
                            filters.append(ScoreBoardFilter::UniquePlayers);
                        }
                        FilterOption::UniqueLanguage => {
                            filters.append(ScoreBoardFilter::UniquePlayers);
                        }
                    };

                    filters.append(ScoreBoardFilter::Sort(
                        SortColumn::from_str(self.sort_column.as_str()).expect("Invalid Cloumn"),
                    ));
                    let scores = ScoreBoard::new(scores.clone()).filter(filters).scores();

                    for (i, score) in scores.iter().enumerate() {
                        let time = NiceTime::new(score.time_ns);
                        let name = score.name.clone();
                        let language = score.language.clone();
                        let binary = score.command.clone();

                        body.row(text_height, |mut row| {
                            row.col(|ui| {
                                ui.label(i.to_string());
                            });
                            row.col(|ui| {
                                ui.label(time.to_string());
                            });
                            row.col(|ui| {
                                ui.label(name);
                            });
                            row.col(|ui| {
                                ui.label(language);
                            });
                            row.col(|ui| {
                                ui.label(binary);
                            });
                        });
                    }
                }
            });
    }
}
