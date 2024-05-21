use std::sync::atomic::AtomicU64;

use crate::gemini;

#[derive(Debug)]
struct Preformatted {
    alt: String,
    contents: String,
}

#[derive(Debug)]
enum GemLine {
    Text(String),
    Heading(u8, String),
    Link(String, String),
    Preformatted(Preformatted),
}

#[derive(Debug, Default)]
pub struct GemText(Vec<(u64, GemLine)>);

static LAST_ID: AtomicU64 = AtomicU64::new(0);

impl GemText {
    pub fn new(contents: &str) -> GemText {
        let contents: String = contents.into();
        
        let mut lines = Vec::new();

        let mut preformatted: Option<Preformatted> = None;

        for line in contents.split("\n") {
            let id = LAST_ID.fetch_add(1, std::sync::atomic::Ordering::AcqRel);

            if let Some(pf) = &mut preformatted {
                if line.starts_with("```") {
                    if pf.contents.ends_with("\n") {
                        pf.contents = pf.contents[..pf.contents.len()-1].into();
                    }
                    lines.push((id, GemLine::Preformatted(preformatted.take().expect("unreachable"))));
                } else {
                    pf.contents += line;
                    pf.contents += "\n";
                }
                continue;
            }

            let line = if line.starts_with("```") {
                preformatted = Some(Preformatted {
                    alt: line[3..].into(),
                    contents: String::new(),
                });
                continue;
            } else if line.starts_with("###") {
                let line = line[3..].trim_start();
                GemLine::Heading(3, line.into())
            } else if line.starts_with("##") {
                let line = line[2..].trim_start();
                GemLine::Heading(2, line.into())
            } else if line.starts_with("#") {
                let line = line[1..].trim_start();
                GemLine::Heading(1, line.into())
            } else if line.starts_with("=>") {
                let line = line[2..].trim_start();
                let i = line.split_whitespace().next().map(|s| s.len()).unwrap_or_default();
                let url = line[..i].trim_end();
                let display = line[i..].trim_start();

                GemLine::Link(url.into(), display.into())
            } else {
                GemLine::Text(line.into())
            };
            lines.push((id, line));
        }

        GemText(lines)
    }

    pub fn raw(contents: impl Into<String>) -> GemText {
        GemText(vec![(LAST_ID.fetch_add(1, std::sync::atomic::Ordering::AcqRel), GemLine::Text(contents.into()))])
    }

    pub fn render(&self, ui: &mut egui::Ui, new_url: &mut Option<String>) {
        for (id, line) in self.0.iter() {
            ui.push_id(id, |ui| {
                match line {
                    GemLine::Text(text) => {
                        ui.label(text);
                    },
                    GemLine::Heading(n, text) => {
                        ui.label(egui::RichText::new(text).size(12.0 + 12.0 / *n as f32));
                    },
                    GemLine::Link(url, display) => {
                        if url.starts_with(&"http://") || url.starts_with(&"https://") {
                            ui.hyperlink_to(format!("\u{1F310} {display}"), url);
                        } else {
                            if ui.link(format!("\u{1F680} {display}")).clicked() {
                                *new_url = Some(url.into());
                            }
                        }
                    }
                    GemLine::Preformatted(Preformatted {
                        // alt,
                        contents,
                        ..
                    }) => {
                        egui::ScrollArea::horizontal().show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(contents).monospace());
                            });
                        });
                    },
                };
            });
        }
    }
}
