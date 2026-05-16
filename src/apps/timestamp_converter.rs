// ABOUTME: Timestamp converter app for translating epoch values and human dates.
// ABOUTME: Renders an egui tool with bidirectional time conversion results.

use chrono::{DateTime, Local, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Utc};
use egui::{RichText, TextEdit, Ui};

#[derive(Default)]
pub struct TimestampConverter {
    input: String,
}

impl TimestampConverter {
    pub fn ui(&mut self, ui: &mut Ui) {
        ui.heading("Timestamp Converter");
        ui.add_space(4.0);
        ui.label("Enter an epoch timestamp or a human readable date-time.");
        ui.add(
            TextEdit::multiline(&mut self.input)
                .desired_width(f32::INFINITY)
                .desired_rows(2)
                .hint_text("1778941965, 1715788800000000000, 2024-05-15T00:00:00Z"),
        );

        ui.horizontal(|ui| {
            if ui.button("Use current time").clicked() {
                self.input = Utc::now().to_rfc3339();
            }
            if ui.button("Clear").clicked() {
                self.input.clear();
            }
        });

        ui.separator();

        match convert_input(&self.input) {
            Ok(Some(result)) => result.ui(ui),
            Ok(None) => {
                ui.label(RichText::new("Waiting for input").weak());
            }
            Err(err) => {
                ui.colored_label(ui.visuals().error_fg_color, err);
            }
        }
    }
}

struct ConversionResult {
    source: String,
    utc: DateTime<Utc>,
}

impl ConversionResult {
    fn ui(&self, ui: &mut Ui) {
        ui.label(RichText::new(&self.source).strong());
        ui.add_space(4.0);

        let local = self.utc.with_timezone(&Local);
        let seconds = self.utc.timestamp();
        let millis = self.utc.timestamp_millis();
        let micros = self.utc.timestamp_micros();
        let nanos = timestamp_nanos(self.utc);

        egui::Grid::new("timestamp_converter_results")
            .num_columns(2)
            .spacing([18.0, 6.0])
            .striped(true)
            .show(ui, |ui| {
                value_row(
                    ui,
                    "UTC",
                    self.utc.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                );
                value_row(ui, "UTC RFC 3339", self.utc.to_rfc3339());
                value_row(
                    ui,
                    "Local",
                    local.format("%Y-%m-%d %H:%M:%S %Z").to_string(),
                );
                value_row(ui, "Local RFC 3339", local.to_rfc3339());
                value_row(ui, "Unix seconds", seconds.to_string());
                value_row(ui, "Unix milliseconds", millis.to_string());
                value_row(ui, "Unix microseconds", micros.to_string());
                value_row(ui, "Unix nanoseconds", nanos.to_string());
            });
    }
}

fn value_row(ui: &mut Ui, label: &str, value: String) {
    ui.label(RichText::new(label).weak());
    ui.monospace(value);
    ui.end_row();
}

fn convert_input(input: &str) -> Result<Option<ConversionResult>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    if let Ok(value) = trimmed.parse::<i128>() {
        let (utc, unit) = epoch_to_datetime(value)?;
        return Ok(Some(ConversionResult {
            source: format!("Parsed as Unix {unit}"),
            utc,
        }));
    }

    let utc = parse_human_datetime(trimmed)?;
    Ok(Some(ConversionResult {
        source: "Parsed as human readable date-time".to_string(),
        utc,
    }))
}

fn epoch_to_datetime(value: i128) -> Result<(DateTime<Utc>, &'static str), String> {
    let abs = value.abs();
    if abs >= 1_000_000_000_000_000_000 {
        datetime_from_parts(
            value.div_euclid(1_000_000_000),
            value.rem_euclid(1_000_000_000) as u32,
            "nanoseconds",
        )
    } else if abs >= 1_000_000_000_000_000 {
        datetime_from_parts(
            value.div_euclid(1_000_000),
            (value.rem_euclid(1_000_000) * 1_000) as u32,
            "microseconds",
        )
    } else if abs >= 1_000_000_000_000 {
        datetime_from_parts(
            value.div_euclid(1_000),
            (value.rem_euclid(1_000) * 1_000_000) as u32,
            "milliseconds",
        )
    } else {
        datetime_from_parts(value, 0, "seconds")
    }
}

fn datetime_from_parts(
    seconds: i128,
    nanos: u32,
    unit: &'static str,
) -> Result<(DateTime<Utc>, &'static str), String> {
    let seconds = i64::try_from(seconds)
        .map_err(|_| format!("Timestamp is outside the supported {unit} range."))?;
    let utc = DateTime::from_timestamp(seconds, nanos)
        .ok_or_else(|| format!("Timestamp is outside the supported {unit} range."))?;
    Ok((utc, unit))
}

fn parse_human_datetime(input: &str) -> Result<DateTime<Utc>, String> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(input) {
        return Ok(dt.with_timezone(&Utc));
    }

    for format in [
        "%Y-%m-%d %H:%M:%S %z",
        "%Y-%m-%d %H:%M %z",
        "%Y-%m-%d %H:%M:%S%.f %z",
        "%Y-%m-%dT%H:%M:%S%z",
        "%Y-%m-%dT%H:%M:%S%.f%z",
    ] {
        if let Ok(dt) = DateTime::parse_from_str(input, format) {
            return Ok(dt.with_timezone(&Utc));
        }
    }

    for format in [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%dT%H:%M:%S%.f",
    ] {
        if let Ok(naive) = NaiveDateTime::parse_from_str(input, format) {
            return local_to_utc(naive);
        }
    }

    if let Ok(date) = NaiveDate::parse_from_str(input, "%Y-%m-%d") {
        let naive = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| "Date could not be converted to midnight.".to_string())?;
        return local_to_utc(naive);
    }

    Err("Enter an epoch number or a date like 2024-05-15T00:00:00Z.".to_string())
}

fn local_to_utc(naive: NaiveDateTime) -> Result<DateTime<Utc>, String> {
    match Local.from_local_datetime(&naive) {
        LocalResult::Single(dt) => Ok(dt.with_timezone(&Utc)),
        LocalResult::Ambiguous(earliest, _) => Ok(earliest.with_timezone(&Utc)),
        LocalResult::None => {
            Err("That local time does not exist in the current timezone.".to_string())
        }
    }
}

fn timestamp_nanos(utc: DateTime<Utc>) -> i128 {
    i128::from(utc.timestamp()) * 1_000_000_000 + i128::from(utc.timestamp_subsec_nanos())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_seconds_epoch() {
        let (dt, unit) = epoch_to_datetime(1_778_941_965).unwrap();
        assert_eq!(unit, "seconds");
        assert_eq!(dt.to_rfc3339(), "2026-05-16T14:32:45+00:00");
    }

    #[test]
    fn parses_nanoseconds_epoch() {
        let (dt, unit) = epoch_to_datetime(1_715_788_800_000_000_000).unwrap();
        assert_eq!(unit, "nanoseconds");
        assert_eq!(dt.to_rfc3339(), "2024-05-15T16:00:00+00:00");
    }

    #[test]
    fn parses_rfc3339_datetime() {
        let dt = parse_human_datetime("2024-05-15T00:00:00Z").unwrap();
        assert_eq!(dt.timestamp(), 1_715_731_200);
    }
}
