#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::egui;
use egui::{FontId, FontFamily};
use egui_file_dialog::FileDialog;
use egui_logger::{TimeFormat, TimePrecision};
use log::LevelFilter;
use regex::Regex;
use std::path::PathBuf;
use url::Url;

use eodms_data_downloader::{download_files, normalize_url, Conf, URL_LUT};

fn main() -> eframe::Result {
    egui_logger::builder()
        .show_all_categories(false)
        .max_level(LevelFilter::Info)
        .init()
        .unwrap();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder {
            maximized: Some(true),
            ..Default::default()
        },
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
    valid_regex: bool,
    file_dialog: FileDialog,
    output_directory: Option<PathBuf>,
    conf: Conf,
    incrx: String,
}

impl eframe::App for MyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            egui::Frame::NONE
                 .show(ui, |ui| {

                if self.incrx.is_empty() {
                    self.valid_regex = true;
                }
                ui.horizontal(|ui| {
                    ui.label("EODMS DDS URL: ");
                    let response =
                        ui.add(egui::TextEdit::singleline(&mut self.conf.url).desired_width(600.0));
                    if response.changed() {
                        match Url::parse(self.conf.url.as_str()) {
                            Ok(_u) => {
                                self.valid_url = true;
                            }
                            Err(_e) => {}
                        }
                    }
                });
                ui.separator();

                ui.horizontal(|ui| {
                    if ui.button("Select Directory").clicked() {
                        self.file_dialog.pick_directory();
                    }
                    if let Some(path) = &self.output_directory {
                        ui.label(format!("Output Directory: {}", path.display()));
                    }
                });
                self.file_dialog.update(ui);

                if let Some(path) = self.file_dialog.take_picked() {
                    self.output_directory = Some(path);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.conf.recursive, "Recursive");
                    ui.checkbox(&mut self.conf.stripdirs, "Strip Directories");
                    ui.label("Number of Threads: ");
                    ui.add(egui::Slider::new(&mut self.conf.num_threads, 1..=16));
                    ui.label("Include Regex: ");
                    let response =
                        ui.add(egui::TextEdit::singleline(&mut self.incrx).desired_width(200.0));
                    if response.changed() {
                        self.valid_regex = Regex::new(&self.incrx).is_ok();
                    }
                });
                ui.separator();
                let submit_button = egui::Button::new(
                    egui::RichText::new("Submit".to_string()).font(FontId { size: 20.0, family: FontFamily::Proportional })
                );
                 //       .frame(false);

                ui.vertical_centered(|ui| {
                    if ui
                        .add_enabled(self.form_is_valid(), submit_button)
                        .clicked()
                    {
                        let mode = &URL_LUT[0];
                        if let Ok(url) = normalize_url(&self.conf.url, mode) {
                            self.conf.url = url;
                            self.conf.output_directory = self
                                .output_directory
                                .as_mut()
                                .unwrap()
                                .clone()
                                .into_os_string()
                                .into_string()
                                .unwrap();
                            if !self.incrx.is_empty() {
                                self.conf.incrx = Some(Regex::new(&self.incrx).unwrap());
                            }
                            match download_files(&self.conf) {
                                Ok(()) => {}
                                Err(e) => {
                                    log::error!("{e:#?}");
                                }
                            }
                        }
                    }
                });
            });

            egui::Frame::NONE.show(ui, |ui| {
                egui_logger::logger_ui()
                    .enable_cache_layouts(true)
                    .enable_categories_button(false)
                    .enable_search(false)
                    .enable_time_button(false)
                    .enable_levels_button(false)
                    .enable_clear_button(false)
                    .enable_max_log_output(false)
                    .enable_regex(false)
                    .enable_autoscroll(true)
                    .enable_category("eodms_data_downloader",true)
                    .align_output(false)
                    .time_format(TimeFormat::Utc)
                    .time_precision(TimePrecision::Milliseconds)
                    .show(ui);
            });
        });
    }
}
impl MyApp {
    fn form_is_valid(&self) -> bool {
        self.output_directory.is_some() && self.valid_url && self.valid_regex
    }
}
