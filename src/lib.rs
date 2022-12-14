#![warn(clippy::all, rust_2018_idioms)]

mod about;
mod app;
mod apps;
pub use app::TemplateApp;

pub trait View {
  fn ui(&mut self, ui: &mut egui::Ui);
}

pub trait AppWindow {
  /// `&'static` so we can also use it as a key to store open/close state.
  fn name(&self) -> &'static str;

  /// Show windows, etc
  fn show(&mut self, ctx: &egui::Context, open: &mut bool);
}