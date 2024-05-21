use std::{io, thread};

use crate::{gemini, gemtext};

pub enum ActionRequired {
    Input {
        prompt: String,
        sensitive: bool,
    },
}

pub struct Tab {
    url: url::Url,
    pub display_url: String,
    title: String,
    content: gemtext::GemText,
    request_thread: Option<thread::JoinHandle<Result<Tab, ActionRequired>>>,
}

impl Tab {
    const MAX_REDIRECTS: usize = 32;

    const BROWSER_SCHEME: &'static str = "about";
    const NEW_URL: &'static str = "about://new";

    const NEW_TEMPLATE: &'static str = include_str!("templates/new.gmi");
    const ERROR_TEMPLATE: &'static str = include_str!("templates/error.gmi");
    
    pub fn new(url: url::Url) -> Tab {
        let title = Self::display_url(&url);
        
        let mut tab = Tab {
            url,
            display_url: title.clone(),
            title,
            content: gemtext::GemText::new(""),
            request_thread: None,
        };

        tab.request(tab.url.clone());
        
        tab
    }
    
    pub fn new_error(url: url::Url, status: u8, error: impl Into<String>) -> Tab {
        let display_url = Self::display_url(&url);
        
        let error: String = error.into();
        
        let template = Self::ERROR_TEMPLATE.replace("{{status}}", &status.to_string())
            .replace("{{message}}", &error);
        
        let content = gemtext::GemText::new(&template);
        
        Tab {
            url,
            display_url,
            title: error,
            content,
            request_thread: None,
        }
    }

    pub fn request(&mut self, mut url: url::Url) {
        self.request_thread = Some(thread::spawn(move || {
            if url.scheme() == Self::BROWSER_SCHEME {
                match url.host_str().unwrap_or_default() {
                    "new" => return Ok(Default::default()),
                    host => return Ok(Tab::new_error(url.clone(), 0, format!("Unknown browser page '{host}'"))),
                }
            }

            let mut redirections = vec![url.clone()];

            let mut out = Tab {
                url: url.clone(),
                display_url: String::new(),
                title: Self::display_url(&url),
                content: Default::default(),
                request_thread: None,
            };
        
            for _ in 0..Self::MAX_REDIRECTS {
                let response = match gemini::request(&url) {
                    Ok(response) => response,
                    Err(err) => return Ok(Tab::new_error(url, 0, err.to_string())),
                };
        
                if let gemini::ResponseContent::Redirection { uri } = response.content() {
                    url = match url.join(&uri) {
                        Ok(url) => url,
                        Err(err) => return Ok(Tab::new_error(url, 0, err.to_string())),
                    };
        
                    if redirections.contains(&url) {
                        // redirect loop
                        break;
                    }
                    
                    redirections.push(url.clone());
                    
                    continue;
                }
    
                match response.content() {
                    gemini::ResponseContent::InputExpected { prompt } => return  Err(ActionRequired::Input { prompt: prompt.into(), sensitive: response.status() == 11 }),
                    gemini::ResponseContent::Success { mimetype, body } => {
                        if mimetype.len() == 0 {
                            return Ok(Tab::new_error(url, 0, io::Error::from(io::ErrorKind::InvalidData).to_string()));
                        }

                        if mimetype.split(';').next().expect("unreachable").trim() == "text/gemini" {
                            out.content = gemtext::GemText::new(&body);
                            let title = body.lines()
                                .filter(|l| l.starts_with("#"))
                                .next()
                                .map(|l| l.strip_prefix(['#', ' ']))
                                .flatten();
                            if let Some(title) = title {
                                let title = title.trim();
                                if title.len() > 0 {
                                    out.title = title.into();
                                }
                            }
                        } else {
                            out.content = gemtext::GemText::raw(body);
                        }
                    },
                    gemini::ResponseContent::TemporaryFailure { error } => {
                        out = Tab::new_error(url, response.status(), error);
                    },
                    gemini::ResponseContent::PermanentFailure { error } => {
                        out = Tab::new_error(url, response.status(), error);
                    },
                    gemini::ResponseContent::ClientCertifiates { error } => todo!("client certificates"),
                    _ => unreachable!(),
                }
    
                break;
            }

            Ok(out)
        }));
    }

    pub fn resolve(&mut self) -> Option<Result<Tab, ActionRequired>> {
        let Some(thread) = &self.request_thread else { return None; };

        if thread.is_finished() {
            let thread = self.request_thread.take().expect("unreachable");
            
            let Ok(mut tab) = thread.join() else { return None; };

            if let Ok(tab) = &mut tab {
                if tab.url.scheme() == Self::BROWSER_SCHEME {
                    tab.display_url = String::new();
                } else {
                    tab.display_url = Self::display_url(&tab.url);
                }
            }

            return Some(tab);
        }
        
        None
    }

    pub fn loading(&self) -> bool {
        self.request_thread.is_some()
    }

    pub fn url(&self) -> &url::Url {
        &self.url
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn content(&self) -> &gemtext::GemText {
        &self.content
    }

    fn display_url(url: &url::Url) -> String {
        if url.scheme() == gemini::SCHEME {
            url.to_string()[url.scheme().len()+3..].into()
        } else {
            url.to_string()
        }
    }
}

impl Default for Tab {
    fn default() -> Self {
        Self {
            url: url::Url::parse(Self::NEW_URL).expect("unreachable"),
            display_url: String::new(),
            title: "New Tab".into(),
            content: gemtext::GemText::new(Tab::NEW_TEMPLATE),
            request_thread: None,
        }
    }
}
