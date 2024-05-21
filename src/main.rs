mod gemtext;
mod gemini;
mod tab;

struct PromptWindow {
    prompt: String,
    sensitive: bool,
    input: String,
}

struct App {
    tabs: Vec<(Vec<tab::Tab>, usize)>,
    current_tab: usize,
    window: Option<PromptWindow>,
    progress: f32,
    target_progress: f32,
}

impl App {
    pub const PROGRESS_APPROACH: f32 = 0.8;

    pub fn new(cc: &eframe::CreationContext) -> App {
        let mut font_defs = egui::FontDefinitions::default();
        font_defs.font_data.insert("ubuntu".into(), egui::FontData::from_static(
            include_bytes!("../fonts/ubuntu_emoji.ttf")
        ));
        font_defs.font_data.insert("ubuntu_mono".into(), egui::FontData::from_static(
            include_bytes!("../fonts/ubuntu_mono.ttf")
        ));
        font_defs.font_data.insert("icons".into(), egui::FontData::from_static(
            include_bytes!("../fonts/icons.ttf")
        ));

        font_defs.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "ubuntu".into());
        font_defs.families.entry(egui::FontFamily::Monospace).or_default().insert(0, "ubuntu_mono".into());
        font_defs.families.entry(egui::FontFamily::Name("icons".into())).or_default().insert(0, "icons".into());
        
        cc.egui_ctx.set_fonts(font_defs);
        cc.egui_ctx.set_style(egui::Style {
            visuals: egui::Visuals {
                dark_mode: true,
                ..Default::default()
            },
            ..Default::default()
        });

        cc.egui_ctx.set_zoom_factor(14.0/12.0);
        
        App {
            tabs: vec![(vec![Default::default()], 0)],
            current_tab: 0,
            window: None,
            progress: 0.0,
            target_progress: 0.0,
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut tab_delta = 0i32;
        
        egui::TopBottomPanel::top("tab_list").show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (i, (history, current)) in self.tabs.iter().enumerate() {
                    if ui.button(history[*current].title()).clicked() {
                        self.current_tab = i;
                    }
                }
                if ui.button(egui::RichText::new("+").family(egui::FontFamily::Name("icons".into()))).clicked() {
                    self.current_tab = self.tabs.len();
                    self.tabs.push((vec![Default::default()], 0));
                }
            });
            let tab = {
                let (history, current) = &mut self.tabs[self.current_tab];
            
                let tab = &mut history[*current];

                // resolve request
                if tab.loading() {
                    if let Some(t) = tab.resolve() {
                        self.target_progress = 1.5;
                        match t {
                            Ok(t) => {
                                // remove future history, new branch
                                *current += 1;
                                history.drain(*current..);
                                history.push(t);
                            },
                            Err(action) => match action {
                                tab::ActionRequired::Input { sensitive, prompt } => self.window = Some(PromptWindow { prompt, sensitive, input: String::new() }),
                            },
                        }
                    }

                    &mut history[*current]
                } else {
                    tab
                }
            };
            
            ui.horizontal(|ui| {
                // if ui.button("\u{2190}").clicked() {
                if ui.button(egui::RichText::new("\u{f060}").family(egui::FontFamily::Name("icons".into()))).clicked() {
                    tab_delta = -1;
                }
                // if ui.button("\u{2192}").clicked() {
                if ui.button(egui::RichText::new("\u{f061}").family(egui::FontFamily::Name("icons".into()))).clicked() {
                    tab_delta = 1;
                }
                // if ui.button("\u{27F3}").clicked() {
                if ui.button(egui::RichText::new("\u{f2f9}").family(egui::FontFamily::Name("icons".into()))).clicked() {
                    tab.request(tab.url().clone());
                    self.target_progress = Self::PROGRESS_APPROACH;
                    self.progress = 0.0;
                }
                let res = ui.add_enabled(!tab.loading(), egui::TextEdit::singleline(&mut tab.display_url).desired_width(f32::INFINITY));
                // pressed enter navigate to url
                if res.lost_focus() && res.ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let url = if !tab.display_url.starts_with(&format!("{}://", gemini::SCHEME)) {
                        format!("{}://{}", gemini::SCHEME, tab.display_url)
                    } else {
                        tab.display_url.clone()
                    };
                    if let Ok(url) = url::Url::parse(&url) {
                        tab.request(url);
                        self.target_progress = Self::PROGRESS_APPROACH;
                        self.progress = 0.0;
                    }
                }
            });
        });
        
        let tab = {
            let (history, current) = &mut self.tabs[self.current_tab];
            // back
            if tab_delta < 0 && *current >= (-tab_delta) as usize {
                *current -= (-tab_delta) as usize;
            }
            // forward
            if tab_delta > 0 && *current + (tab_delta as usize) < history.len() {
                *current += tab_delta as usize;
            }

            &mut history[*current]
        };

        let rate = 1.0 / if self.target_progress > 0.999 {
            // move fast, page loaded
            10.0
        } else {
            // page still loading, move slowly
            200.0
        };
        
        // loading failed
        if self.target_progress < Self::PROGRESS_APPROACH + 0.01 && !tab.loading() {
            self.target_progress = 0.0;
            // loading finished
        } else if self.progress > 0.999 {
            self.target_progress = 0.0;
            self.progress = 0.0;
        }
        self.progress += (self.target_progress - self.progress) * rate;

        egui::CentralPanel::default().frame(egui::Frame::default().inner_margin(egui::Margin::ZERO).fill(egui::Color32::from_gray(10))).show(ctx, |ui| {
            ui.add(egui::ProgressBar::new(self.progress).rounding(egui::Rounding::default()).desired_height(2.0).animate(true));
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                let margin = ((ui.available_width() - 800.0) / 2.0).max(8.0);
                
                egui::Frame::default().outer_margin(egui::Margin::symmetric(margin, 4.0)).show(ui, |ui| {let mut new_url = None;
                    tab.content().render(ui, &mut new_url);
                    
                    if let Some(url) = new_url {
                        if let Ok(url) = tab.url().join(&url) {
                            tab.request(url);
                            self.target_progress = Self::PROGRESS_APPROACH;
                            self.progress = 0.0;
                        }
                    }
                });
            });
        });

        let mut close_window = false;
        if let Some(window) = &mut self.window {
            egui::Window::new(egui::RichText::new(&window.prompt).text_style(egui::TextStyle::Body)).show(ctx, |ui| {
                ui.add(egui::TextEdit::multiline(&mut window.input).desired_width(f32::INFINITY).desired_rows(1).password(window.sensitive));
                ui.horizontal(|ui| {
                    if ui.button("Submit").clicked() {
                        close_window = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close_window = true;
                    }
                });
            });
        }
    }
}

fn main() {
    env_logger::init();
    
    if let Err(err) = eframe::run_native("Vostok", eframe::NativeOptions::default(), Box::new(|cc| Box::new(App::new(cc)))) {
        eprintln!("{err}");
    }
}
