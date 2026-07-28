#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::egui;
use egui_file_dialog::FileDialog;
use std::path::PathBuf;
use url::Url;

use eodms_data_downloader::{Conf, download_files, normalize_url, URL_LUT};

fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 340.0]),
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "EODMS Data Downloader",
        options,
        Box::new(|_cc| Ok(Box::<MyApp>::default())),
    )
}

#[derive(Default)]
struct MyApp {
    valid_url: bool,
    file_dialog: FileDialog,
    output_directory: Option<PathBuf>,
    conf: Conf,
}

impl eframe::App for MyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.vertical_centered(|u| u.heading("EODMS Data Downloader"));
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("EODMS DDS URL: ");
                let response = ui.add(egui::TextEdit::singleline(&mut self.conf.url).desired_width(600.0));
                if response.changed() {
                    match Url::parse(self.conf.url.as_str()) {
                        Ok(_u) => {
                            self.valid_url = true;
                        },
                        Err(_e) => {}
                    }
                }
            });
            ui.separator();

            if let Some(path) = &self.output_directory {
                ui.label(format!("Output Directory: {}", path.display()));
            }

            if ui.button("Select Directory").clicked() {
                self.file_dialog.pick_directory(); 
            }

            self.file_dialog.update(ui);

            if let Some(path) = self.file_dialog.take_picked() {
                self.output_directory = Some(path);
            }
            ui.separator();
            /*
    pub num_threads: usize,
    pub incrx: Option<regex::Regex>,
    */
            ui.checkbox(&mut self.conf.recursive, "Recursive");
            ui.separator();
            ui.checkbox(&mut self.conf.stripdirs, "Strip Directories");
            ui.separator();

            if ui.add_enabled(self.form_is_valid(), egui::Button::new("Submit")).clicked() {
                let mode = &URL_LUT[0];
                if let Ok(url) = normalize_url(&self.conf.url, mode) {
                    self.conf.url = url;
                    self.conf.output_directory = self.output_directory.as_mut().unwrap().clone().into_os_string().into_string().unwrap();
                    /*
                    let conf = Conf {
                        output_directory: self.output_directory.as_mut().unwrap().clone().into_os_string().into_string().unwrap(),
                        url,
                        ..Default::default()
                    };
                    */
                    match download_files(&self.conf) {
                        Ok(()) => {},
                        Err(e) => {
                            // popup
                            log::error!("{e:#?}");
                        }
                    }
                }
            }

        });
    }
}
impl MyApp {
    fn form_is_valid(&self) -> bool {
        self.output_directory.is_some() && self.valid_url
    }
}
