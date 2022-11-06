use egui::Ui;

#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(default))]
pub struct AboutMe {}

impl Default for AboutMe {
    fn default() -> Self {
        Self {}
    }
}

impl AboutMe {
    pub fn ui(&mut self, ui: &mut Ui) {
        egui::TopBottomPanel::top("top_panel")
        .resizable(true)
        .min_height(32.0)
        .show_inside(ui, |ui| {
          egui::ScrollArea::vertical().show(ui, |ui| {
              ui.with_layout(
                  egui::Layout::top_down(egui::Align::LEFT).with_cross_justify(true),
                  |ui| {
                      if ui.label(
                          egui::RichText::new("My name is Josh and I do DevOPs for a living. These are some Rust egui tests.").weak(),
                      ).double_clicked() {
                          //
                      }
                  },
              );
          });
        });
        // ui.label("Life Skills");
        // egui::Grid::new("life_skills")
        //     .num_columns(2)
        //     .spacing([40.0, 4.0])
        //     .striped(true)
        //     .show(ui, |ui| {
        //         ui.label("Drawing");
        //         let progress = 300.0 / 360.0;
        //         let progress_bar = egui::ProgressBar::new(progress).show_percentage();
        //         ui.add(progress_bar);
        //         ui.end_row();
        //         ui.label("Painting");
        //         let progress = 250.0 / 360.0;
        //         let progress_bar = egui::ProgressBar::new(progress).show_percentage();
        //         ui.add(progress_bar);
        //         ui.end_row();
        //         ui.label("Cooking");
        //         let progress = 290.0 / 360.0;
        //         let progress_bar = egui::ProgressBar::new(progress).show_percentage();
        //         ui.add(progress_bar);
        //         ui.end_row();
        //         ui.label("Model Building: Plastic Models");
        //         let progress = 200.0 / 360.0;
        //         let progress_bar = egui::ProgressBar::new(progress).show_percentage();
        //         ui.add(progress_bar);
        //         ui.end_row();
        //     });
        // ui.separator();
        ui.label("Work Skills");
        egui::Grid::new("work_skills")
            .num_columns(2)
            .spacing([150.0, 4.0])
            .striped(true)
            .show(ui, |ui| {
                ui.label("Python");
                let progress = 234.0 / 360.0;
                let progress_bar = egui::ProgressBar::new(progress).show_percentage();
                ui.add(progress_bar);
                ui.end_row();
                ui.label("Javascript");
                let progress = 126.0 / 360.0;
                let progress_bar = egui::ProgressBar::new(progress).show_percentage();
                ui.add(progress_bar);
                ui.end_row();
                ui.label("Rust");
                let progress = 60.0 / 360.0;
                let progress_bar = egui::ProgressBar::new(progress).show_percentage();
                ui.add(progress_bar);
                ui.end_row();
                ui.label("Elixir");
                let progress = 85.0 / 360.0;
                let progress_bar = egui::ProgressBar::new(progress).show_percentage();
                ui.add(progress_bar);
                ui.end_row();
            });
    }
}
