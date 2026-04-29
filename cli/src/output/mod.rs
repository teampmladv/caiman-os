//! output/mod.rs — Terminal output: tables, JSON, YAML, colored text

use anyhow::Result;
use clap::ValueEnum;
use colored::*;
use comfy_table::{Table, Cell, Attribute, Color as TColor, ContentArrangement};

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
    Yaml,
    Wide,
    Jsonl,
}

// ── Caimán color palette ──────────────────────────────────────────────────

pub fn bright(s: &str)  -> ColoredString { s.truecolor(118, 255, 3) }
pub fn green(s: &str)   -> ColoredString { s.truecolor(76,  175, 80) }
pub fn dim(s: &str)     -> ColoredString { s.truecolor(74,  124, 74) }
pub fn amber(s: &str)   -> ColoredString { s.truecolor(255, 179, 0) }
pub fn red(s: &str)     -> ColoredString { s.truecolor(239, 83,  80) }
pub fn blue(s: &str)    -> ColoredString { s.truecolor(66,  165, 245) }
pub fn white(s: &str)   -> ColoredString { s.truecolor(200, 230, 201) }

// ── Status coloring ───────────────────────────────────────────────────────

pub fn color_status(status: &str) -> ColoredString {
    match status {
        "RUNNING"   => bright(status),
        "MIGRATING" => blue(status),
        "BOOTING"   => amber(status),
        "STOPPED"   => dim(status),
        "ERROR"     => red(status),
        "HEALTHY"   => green(status),
        "HIGH_LOAD" => amber(status),
        "CRITICAL"  => red(status),
        "OFFLINE"   => red(status),
        "ALLOW"     => bright("ALLOW"),
        "DENY"      => red("DENY"),
        "LOG"       => amber("LOG"),
        _           => white(status),
    }
}

pub fn color_pct(pct: f64) -> ColoredString {
    let s = format!("{:.0}%", pct);
    if pct > 80.0 { red(&s) }
    else if pct > 60.0 { amber(&s) }
    else { green(&s) }
}

pub fn color_sigma(sigma: f64) -> ColoredString {
    let s = format!("{:.3}", sigma);
    if sigma > 0.20 { red(&s) }
    else if sigma > 0.10 { amber(&s) }
    else { bright(&s) }
}

pub fn color_score(score: f64) -> ColoredString {
    let s = format!("{:.2}", score);
    if score >= 0.70 { bright(&s) }
    else if score >= 0.40 { green(&s) }
    else { dim(&s) }
}

// ── Table builder ─────────────────────────────────────────────────────────

pub fn new_table(headers: &[&str]) -> Table {
    let mut t = Table::new();
    t.set_content_arrangement(ContentArrangement::Dynamic);
    t.load_preset(comfy_table::presets::UTF8_BORDERS_ONLY);

    let header_cells: Vec<Cell> = headers.iter().map(|h| {
        Cell::new(h.to_uppercase())
            .add_attribute(Attribute::Bold)
            .fg(TColor::DarkGreen)
    }).collect();

    t.set_header(header_cells);
    t
}

// ── JSON / YAML output ─────────────────────────────────────────────────────

pub fn format_json(v: &serde_json::Value, fmt: OutputFormat) -> Result<String> {
    match fmt {
        OutputFormat::Json | OutputFormat::Wide => {
            Ok(highlight_json(&serde_json::to_string_pretty(v)?))
        }
        OutputFormat::Yaml => {
            Ok(serde_yaml::to_string(v)?)
        }
        OutputFormat::Jsonl => {
            Ok(v.to_string())
        }
        OutputFormat::Table => {
            // Fallback: pretty JSON when table format is not available
            Ok(highlight_json(&serde_json::to_string_pretty(v)?))
        }
    }
}

fn highlight_json(s: &str) -> String {
    // Simple coloring without syntect dependency on CI
    let mut out = String::new();
    for line in s.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('"') && trimmed.contains("\": ") {
            // Key: value
            if let Some(colon_pos) = trimmed.find("\": ") {
                let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
                let key = &trimmed[..colon_pos + 1];
                let val = &trimmed[colon_pos + 3..];
                let colored_val = if val.starts_with('"') {
                    green(val.trim_end_matches(',').trim_matches('"')).to_string()
                } else if val == "true" || val == "false" {
                    blue(val).to_string()
                } else if val.starts_with(|c: char| c.is_numeric() || c == '-') {
                    amber(val.trim_end_matches(',')).to_string()
                } else {
                    val.to_string()
                };
                out.push_str(&format!("{}{}: {}", indent, dim(key), colored_val));
            } else {
                out.push_str(line);
            }
        } else {
            out.push_str(line);
        }
        out.push('\n');
    }
    out
}

// ── Progress bar helpers ───────────────────────────────────────────────────

pub fn mini_bar(pct: f64, width: usize) -> String {
    let filled = ((pct / 100.0) * width as f64) as usize;
    let empty  = width.saturating_sub(filled);
    format!("[{}{}]",
        bright(&"█".repeat(filled)).to_string(),
        dim(&"░".repeat(empty)).to_string()
    )
}

pub fn format_bytes(b: u64) -> String {
    if b >= 1 << 30 { format!("{:.1} GiB", b as f64 / (1u64 << 30) as f64) }
    else if b >= 1 << 20 { format!("{:.1} MiB", b as f64 / (1u64 << 20) as f64) }
    else if b >= 1 << 10 { format!("{:.1} KiB", b as f64 / (1u64 << 10) as f64) }
    else { format!("{b} B") }
}

pub fn format_uptime(secs: u64) -> String {
    let d = secs / 86400;
    let h = (secs % 86400) / 3600;
    let m = (secs % 3600) / 60;
    if d > 0 { format!("{}d {}h", d, h) }
    else if h > 0 { format!("{}h {}m", h, m) }
    else { format!("{}m", m) }
}

// ── Caiman logo ───────────────────────────────────────────────────────────

pub fn print_logo() {
    println!("{}", bright("
   ██████╗ █████╗ ██╗███╗   ███╗ █████╗ ███╗   ██╗
  ██╔════╝██╔══██╗██║████╗ ████║██╔══██╗████╗  ██║
  ██║     ███████║██║██╔████╔██║███████║██╔██╗ ██║
  ██║     ██╔══██║██║██║╚██╔╝██║██╔══██║██║╚██╗██║
  ╚██████╗██║  ██║██║██║ ╚═╝ ██║██║  ██║██║ ╚████║
   ╚═════╝╚═╝  ╚═╝╚═╝╚═╝     ╚═╝╚═╝  ╚═╝╚═╝  ╚═══╝"));
    println!("  {}", dim("Named after the Cuban crocodile. Built for the cloud."));
    println!();
}
