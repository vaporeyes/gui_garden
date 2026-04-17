use std::sync::Arc;

use egui::{Color32, RichText, TextFormat, Ui};

use super::note::{Note, NoteLink};
use super::note_directory::NoteDirectory;

/// Type of Markdown block
#[derive(Debug, Clone, PartialEq)]
pub enum BlockType {
    Paragraph,
    Heading(usize),
    CodeBlock(String),
    MermaidDiagram,
    BlockQuote,
    Callout(String),
    BulletList,
    NumberedList,
    Table,
    ThematicBreak,
}

/// Type of inline element
#[derive(Debug, Clone, PartialEq)]
pub enum InlineType {
    Text,
    Bold,
    Italic,
    Strikethrough,
    Code,
    Link(String),
    InternalLink(NoteLink),
    Highlight,
    Image(String),
    InlineLatex,
}

/// A block of Markdown content
#[derive(Debug, Clone)]
pub struct MarkdownBlock {
    pub block_type: BlockType,
    pub content: String,
    pub inlines: Vec<MarkdownInline>,
    pub children: Vec<MarkdownBlock>,
}

/// An inline element within a block
#[derive(Debug, Clone)]
pub struct MarkdownInline {
    pub inline_type: InlineType,
    pub content: String,
}

/// The parsed Markdown document
#[derive(Debug, Clone)]
pub struct ParsedMarkdown {
    pub blocks: Vec<MarkdownBlock>,
}

/// Parse Markdown content
pub fn parse_markdown(content: &str) -> ParsedMarkdown {
    // Split content into lines for processing
    let lines: Vec<&str> = content.lines().collect();

    let blocks = parse_blocks(&lines);

    ParsedMarkdown { blocks }
}

/// Parse Markdown blocks
fn parse_blocks(lines: &[&str]) -> Vec<MarkdownBlock> {
    let mut blocks = Vec::new();
    let mut current_lines = Vec::new();
    let mut current_block_type = BlockType::Paragraph;
    let mut in_code_block = false;
    let mut code_language = String::new();

    for line in lines {
        // Check for code block boundaries
        if line.starts_with("```") {
            if in_code_block {
                // End of code block
                let content = current_lines.join("\n");
                let inlines = vec![MarkdownInline {
                    inline_type: InlineType::Text,
                    content: content.clone(),
                }];

                blocks.push(MarkdownBlock {
                    block_type: if code_language == "mermaid" {
                        BlockType::MermaidDiagram
                    } else {
                        BlockType::CodeBlock(code_language.clone())
                    },
                    content,
                    inlines,
                    children: Vec::new(),
                });

                current_lines.clear();
                in_code_block = false;
                code_language = String::new();
                current_block_type = BlockType::Paragraph;
            } else {
                // Start of code block
                if !current_lines.is_empty() {
                    process_current_block(&mut blocks, &current_lines, &current_block_type);
                    current_lines.clear();
                }

                in_code_block = true;
                let language = line.trim_start_matches("```").trim();
                code_language = language.to_string();
                current_block_type = BlockType::CodeBlock(code_language.clone());
            }
            continue;
        }

        if in_code_block {
            current_lines.push(line);
            continue;
        }

        // Check for heading
        if line.starts_with('#') {
            if !current_lines.is_empty() {
                process_current_block(&mut blocks, &current_lines, &current_block_type);
                current_lines.clear();
            }

            let heading_level = line.chars().take_while(|&c| c == '#').count();
            current_block_type = BlockType::Heading(heading_level);
            current_lines.push(line.trim_start_matches(|c| c == '#').trim_start());

            process_current_block(&mut blocks, &current_lines, &current_block_type);
            current_lines.clear();
            current_block_type = BlockType::Paragraph;
            continue;
        }

        // Check for thematic break
        if line.trim() == "---" || line.trim() == "***" || line.trim() == "___" {
            if !current_lines.is_empty() {
                process_current_block(&mut blocks, &current_lines, &current_block_type);
                current_lines.clear();
            }

            blocks.push(MarkdownBlock {
                block_type: BlockType::ThematicBreak,
                content: line.to_string(),
                inlines: Vec::new(),
                children: Vec::new(),
            });

            current_block_type = BlockType::Paragraph;
            continue;
        }

        // Check for blockquote (and callouts)
        if line.starts_with('>') {
            if current_block_type != BlockType::BlockQuote
                && current_block_type != BlockType::Callout(String::new())
            {
                if !current_lines.is_empty() {
                    process_current_block(&mut blocks, &current_lines, &current_block_type);
                    current_lines.clear();
                }

                let content = line.trim_start_matches('>').trim_start();

                // Check for callout syntax
                if content.starts_with("[!") && content.contains(']') {
                    let end_pos = content.find(']').unwrap();
                    let callout_type = content[2..end_pos].to_string();
                    current_block_type = BlockType::Callout(callout_type);
                    current_lines.push(&content[end_pos + 1..].trim_start());
                } else {
                    current_block_type = BlockType::BlockQuote;
                    current_lines.push(content);
                }
            } else {
                current_lines.push(line.trim_start_matches('>').trim_start());
            }
            continue;
        } else if current_block_type == BlockType::BlockQuote
            || current_block_type == BlockType::Callout(String::new())
        {
            // End of blockquote or callout
            process_current_block(&mut blocks, &current_lines, &current_block_type);
            current_lines.clear();
            current_block_type = BlockType::Paragraph;
        }

        // Check for bullet list
        if line.trim().starts_with("- ") || line.trim().starts_with("* ") {
            if current_block_type != BlockType::BulletList {
                if !current_lines.is_empty() {
                    process_current_block(&mut blocks, &current_lines, &current_block_type);
                    current_lines.clear();
                }
                current_block_type = BlockType::BulletList;
            }
            current_lines.push(line);
            continue;
        } else if current_block_type == BlockType::BulletList && line.trim().is_empty() {
            // End of bullet list
            process_current_block(&mut blocks, &current_lines, &current_block_type);
            current_lines.clear();
            current_block_type = BlockType::Paragraph;
        }

        // Check for numbered list
        if line.trim().starts_with(|c: char| c.is_ascii_digit()) && line.contains(". ") {
            if current_block_type != BlockType::NumberedList {
                if !current_lines.is_empty() {
                    process_current_block(&mut blocks, &current_lines, &current_block_type);
                    current_lines.clear();
                }
                current_block_type = BlockType::NumberedList;
            }
            current_lines.push(line);
            continue;
        } else if current_block_type == BlockType::NumberedList && line.trim().is_empty() {
            // End of numbered list
            process_current_block(&mut blocks, &current_lines, &current_block_type);
            current_lines.clear();
            current_block_type = BlockType::Paragraph;
        }

        // Empty line marks the end of the current block (unless it's already empty)
        if line.trim().is_empty() {
            if !current_lines.is_empty() {
                process_current_block(&mut blocks, &current_lines, &current_block_type);
                current_lines.clear();
                current_block_type = BlockType::Paragraph;
            }
            continue;
        }

        // Just add the line to the current block
        current_lines.push(line);
    }

    // Process any remaining content
    if !current_lines.is_empty() {
        process_current_block(&mut blocks, &current_lines, &current_block_type);
    }

    blocks
}

/// Process the current block and add it to the blocks list
fn process_current_block(blocks: &mut Vec<MarkdownBlock>, lines: &[&str], block_type: &BlockType) {
    let content = lines.join("\n");
    let inlines = parse_inlines(&content, block_type);

    blocks.push(MarkdownBlock {
        block_type: block_type.clone(),
        content,
        inlines,
        children: Vec::new(),
    });
}

/// Parse inline elements
fn parse_inlines(content: &str, block_type: &BlockType) -> Vec<MarkdownInline> {
    let mut inlines = Vec::new();
    let mut start = 0;

    // Process text
    inlines.push(MarkdownInline {
        inline_type: InlineType::Text,
        content: content.to_string(),
    });

    // This is simplified - in a real implementation, we'd parse for bolding, italics, etc.
    // Would extract and process things like [[internal links]], ==highlights==, etc.

    inlines
}

/// Render a parsed markdown document to egui
pub fn render_markdown(
    ui: &mut Ui,
    parsed: &ParsedMarkdown,
    note_directory: &NoteDirectory,
    current_note_id: &str,
    on_link_click: &mut impl FnMut(&str),
) {
    for block in &parsed.blocks {
        render_block(ui, block, note_directory, current_note_id, on_link_click);
    }
}

/// Render a markdown block
fn render_block(
    ui: &mut Ui,
    block: &MarkdownBlock,
    note_directory: &NoteDirectory,
    current_note_id: &str,
    on_link_click: &mut impl FnMut(&str),
) {
    match &block.block_type {
        BlockType::Paragraph => {
            ui.label(block.content.clone());
        }
        BlockType::Heading(level) => {
            let size = match level {
                1 => 32.0,
                2 => 24.0,
                3 => 20.0,
                4 => 18.0,
                5 => 16.0,
                _ => 14.0,
            };
            ui.add_space(8.0);
            ui.heading(RichText::new(&block.content).size(size));
            ui.add_space(8.0);
        }
        BlockType::CodeBlock(language) => {
            ui.add_space(4.0);
            let code_bg_color = ui.visuals().code_bg_color;
            egui::Frame::none()
                .fill(code_bg_color)
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    if !language.is_empty() {
                        ui.label(RichText::new(language).small().weak());
                        ui.separator();
                    }
                    ui.monospace(&block.content);
                });
            ui.add_space(4.0);
        }
        BlockType::MermaidDiagram => {
            ui.add_space(4.0);
            let code_bg_color = ui.visuals().code_bg_color;
            egui::Frame::none()
                .fill(code_bg_color)
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.label(RichText::new("mermaid").small().weak());
                    ui.separator();
                    ui.monospace("Mermaid diagram (rendering not implemented yet)");
                    ui.monospace(&block.content);
                });
            ui.add_space(4.0);
        }
        BlockType::BlockQuote => {
            ui.add_space(4.0);
            egui::Frame::none()
                .fill(ui.visuals().faint_bg_color)
                .inner_margin(egui::Margin::same(8))
                .stroke(egui::Stroke::new(
                    4.0,
                    ui.visuals().text_color().gamma_multiply(0.5),
                ))
                .show(ui, |ui| {
                    ui.label(&block.content);
                });
            ui.add_space(4.0);
        }
        BlockType::Callout(callout_type) => {
            ui.add_space(4.0);

            // Choose color based on callout type
            let color = match callout_type.as_str() {
                "note" => Color32::from_rgb(79, 169, 255),    // Blue
                "info" => Color32::from_rgb(79, 169, 255),    // Blue
                "tip" => Color32::from_rgb(68, 201, 127),     // Green
                "warning" => Color32::from_rgb(255, 181, 77), // Orange
                "danger" => Color32::from_rgb(255, 107, 107), // Red
                _ => Color32::from_rgb(150, 150, 150),        // Gray
            };

            egui::Frame::none()
                .fill(color.gamma_multiply(0.15))
                .stroke(egui::Stroke::new(4.0, color))
                .inner_margin(egui::Margin::same(8))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Show icon for callout type
                        let icon = match callout_type.as_str() {
                            "note" => "ℹ️",
                            "info" => "ℹ️",
                            "tip" => "💡",
                            "warning" => "⚠️",
                            "danger" => "🔥",
                            _ => "📌",
                        };

                        ui.label(RichText::new(icon).size(20.0));
                        ui.label(RichText::new(callout_type).strong());
                    });

                    ui.separator();
                    ui.label(&block.content);
                });
            ui.add_space(4.0);
        }
        BlockType::BulletList => {
            ui.add_space(4.0);
            for line in block.content.lines() {
                let line = line.trim().trim_start_matches(['-', '*']).trim_start();
                ui.horizontal(|ui| {
                    ui.label("•");
                    ui.label(line);
                });
            }
            ui.add_space(4.0);
        }
        BlockType::NumberedList => {
            ui.add_space(4.0);
            for line in block.content.lines() {
                if let Some(index) = line.find(". ") {
                    let number = &line[..index + 1];
                    let content = &line[index + 2..];
                    ui.horizontal(|ui| {
                        ui.label(number);
                        ui.label(content);
                    });
                } else {
                    ui.label(line);
                }
            }
            ui.add_space(4.0);
        }
        BlockType::Table => {
            // Simplified table rendering - actual implementation would be more complex
            ui.label("Table (rendering not implemented yet)");
            ui.monospace(&block.content);
        }
        BlockType::ThematicBreak => {
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
        }
    }

    // Render any children blocks
    for child in &block.children {
        render_block(ui, child, note_directory, current_note_id, on_link_click);
    }
}

/// Render inline elements
fn render_inlines(
    ui: &mut Ui,
    inlines: &[MarkdownInline],
    note_directory: &NoteDirectory,
    current_note_id: &str,
    on_link_click: &mut impl FnMut(&str),
) {
    for inline in inlines {
        match &inline.inline_type {
            InlineType::Text => {
                ui.label(&inline.content);
            }
            InlineType::Bold => {
                ui.label(RichText::new(&inline.content).strong());
            }
            InlineType::Italic => {
                ui.label(RichText::new(&inline.content).italics());
            }
            InlineType::Strikethrough => {
                ui.label(RichText::new(&inline.content).strikethrough());
            }
            InlineType::Code => {
                ui.monospace(&inline.content);
            }
            InlineType::Link(url) => {
                ui.hyperlink_to(&inline.content, url);
            }
            InlineType::InternalLink(link) => {
                let target_id = &link.target_id;
                if let Some(target_note) = note_directory.get_note(target_id) {
                    if ui.link(&link.display_text).clicked() {
                        on_link_click(&target_note.id);
                    }
                } else {
                    // Link to non-existent note
                    ui.label(
                        RichText::new(&link.display_text).color(Color32::from_rgb(200, 100, 100)),
                    );
                }
            }
            InlineType::Highlight => {
                let text_color = ui.visuals().text_color();
                let highlight_color = Color32::from_rgb(255, 255, 100).gamma_multiply(0.5);
                let text = RichText::new(&inline.content)
                    .color(text_color)
                    .background_color(highlight_color);
                ui.label(text);
            }
            InlineType::Image(url) => {
                ui.label(format!("[Image: {}]", url));
                // In a real implementation, we would load and display the image
            }
            InlineType::InlineLatex => {
                ui.monospace(format!("${}$", &inline.content));
                // In a real implementation, we would render the LaTeX equation
            }
        }
    }
}

/// Process Obsidian-style links in a note and create navigation
pub fn process_internal_links(
    ui: &mut Ui,
    note: &Note,
    note_directory: &NoteDirectory,
    on_link_click: &mut impl FnMut(&str),
) {
    for link in &note.links {
        if let Some(target_note) = note_directory.get_note(&link.target_id) {
            if link.is_embed {
                // Embed the note content
                ui.add_space(8.0);
                egui::Frame::none()
                    .fill(ui.visuals().faint_bg_color)
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        ui.heading(format!("Embedded: {}", target_note.title()));
                        ui.separator();
                        // Parse and render the embedded note
                        let parsed = parse_markdown(&target_note.content);
                        render_markdown(
                            ui,
                            &parsed,
                            note_directory,
                            &target_note.id,
                            on_link_click,
                        );
                    });
                ui.add_space(8.0);
            }
        }
    }
}
