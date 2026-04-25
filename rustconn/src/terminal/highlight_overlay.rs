//! Overlay-based colored highlight rendering for VTE terminals.
//!
//! VTE's `match_add_regex()` only shows underlines on hover — it does not
//! support custom foreground/background colors.  This module draws colored
//! rectangles and underlines on a transparent `gtk4::DrawingArea` layered
//! on top of the terminal via `gtk4::Overlay`.
//!
//! ## Architecture
//!
//! 1. [`HighlightOverlay::new`] creates a `DrawingArea` and attaches it as
//!    an overlay on the provided `gtk4::Overlay` widget.
//! 2. [`HighlightOverlay::connect`] wires VTE's `contents-changed` signal
//!    so the overlay repaints whenever terminal output changes.
//! 3. On each paint the overlay reads the visible text via
//!    `terminal.text_range_format()`, runs [`CompiledHighlightRules::find_matches`]
//!    per line, and draws colored rectangles (background) and underlines
//!    (foreground) using Cairo.

use gtk4::prelude::*;
use gtk4::{DrawingArea, Overlay};
use rustconn_core::highlight::CompiledHighlightRules;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use uuid::Uuid;
use vte4::Terminal;
use vte4::prelude::*;

/// A transparent drawing layer that renders colored highlight matches
/// on top of a VTE terminal.
pub struct HighlightOverlay {
    drawing_area: DrawingArea,
}

/// Parses a CSS hex color string (`#RRGGBB`) into `(r, g, b)` floats in 0.0–1.0.
fn parse_hex_color(hex: &str) -> Option<(f64, f64, f64)> {
    let hex = hex.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((
        f64::from(r) / 255.0,
        f64::from(g) / 255.0,
        f64::from(b) / 255.0,
    ))
}

impl HighlightOverlay {
    /// Creates a new highlight overlay and attaches it to the given `Overlay` widget.
    ///
    /// The `DrawingArea` is set to transparent (pass-through for mouse events)
    /// so it does not interfere with VTE's own input handling.
    pub fn new(overlay: &Overlay, terminal: &Terminal) -> Self {
        let drawing_area = DrawingArea::new();
        drawing_area.set_hexpand(true);
        drawing_area.set_vexpand(true);
        // Let mouse events pass through to the terminal underneath
        drawing_area.set_can_target(false);

        overlay.add_overlay(&drawing_area);

        // Initial empty draw function — replaced by `connect()`
        let term_weak = terminal.downgrade();
        drawing_area.set_draw_func(move |_da, cr, _w, _h| {
            cr.set_operator(gtk4::cairo::Operator::Clear);
            let _ = cr.paint();
            cr.set_operator(gtk4::cairo::Operator::Over);
            let _ = term_weak.upgrade();
        });

        Self { drawing_area }
    }

    /// Wires the overlay to repaint on every `contents-changed` signal from VTE.
    ///
    /// `rules` is the shared compiled highlight rules map for all sessions.
    pub fn connect(
        &self,
        terminal: &Terminal,
        rules: Rc<RefCell<HashMap<Uuid, CompiledHighlightRules>>>,
        session_id: Uuid,
    ) {
        let da = self.drawing_area.clone();
        let term_for_draw = terminal.clone();
        let rules_for_draw = rules;

        self.drawing_area
            .set_draw_func(move |_da, cr, width, height| {
                // Clear to fully transparent
                cr.set_operator(gtk4::cairo::Operator::Clear);
                if cr.paint().is_err() {
                    return;
                }
                cr.set_operator(gtk4::cairo::Operator::Over);

                let rules_map = rules_for_draw.borrow();
                let Some(compiled) = rules_map.get(&session_id) else {
                    return;
                };

                let row_count = term_for_draw.row_count();
                let col_count = term_for_draw.column_count();
                if row_count <= 0 || col_count <= 0 {
                    return;
                }

                // Compute cell dimensions from the terminal's visible area
                let cell_w = f64::from(width) / col_count as f64;
                let cell_h = f64::from(height) / row_count as f64;
                if cell_w <= 0.0 || cell_h <= 0.0 {
                    return;
                }

                for row in 0..row_count {
                    let (line_opt, _) =
                        term_for_draw.text_range_format(vte4::Format::Text, row, 0, row, col_count);
                    let Some(line_gstr) = line_opt else {
                        continue;
                    };
                    let line = line_gstr.as_str();
                    if line.is_empty() {
                        continue;
                    }

                    let matches = compiled.find_matches(line);
                    if matches.is_empty() {
                        continue;
                    }

                    let y = row as f64 * cell_h;

                    for m in &matches {
                        // Convert byte offsets to column positions
                        let col_start = line[..m.start].chars().count();
                        let col_end = line[..m.end].chars().count();
                        let x = col_start as f64 * cell_w;
                        let w = (col_end - col_start) as f64 * cell_w;

                        // Draw background highlight rectangle
                        if let Some(ref bg) = m.background_color
                            && let Some((r, g, b)) = parse_hex_color(bg)
                        {
                            cr.set_source_rgba(r, g, b, 0.35);
                            cr.rectangle(x, y, w, cell_h);
                            if cr.fill().is_err() {
                                return;
                            }
                        }

                        // Draw foreground colored underline (2px thick)
                        if let Some(ref fg) = m.foreground_color
                            && let Some((r, g, b)) = parse_hex_color(fg)
                        {
                            cr.set_source_rgba(r, g, b, 0.9);
                            cr.set_line_width(2.0);
                            cr.move_to(x, y + cell_h - 1.0);
                            cr.line_to(x + w, y + cell_h - 1.0);
                            if cr.stroke().is_err() {
                                return;
                            }
                        }
                    }
                }
            });

        // Throttled redraw on contents-changed — avoid excessive Cairo
        // rendering during fast terminal output (e.g. `cat /dev/urandom`).
        // Uses a 32ms (~30fps) idle-based throttle: the first event queues
        // a draw, subsequent events within the window are coalesced.
        let da_for_signal = da;
        let redraw_pending = Rc::new(std::cell::Cell::new(false));
        let redraw_pending_signal = redraw_pending.clone();
        terminal.connect_contents_changed(move |_| {
            if redraw_pending_signal.get() {
                return; // Already scheduled
            }
            redraw_pending_signal.set(true);
            let da_idle = da_for_signal.clone();
            let pending = redraw_pending_signal.clone();
            gtk4::glib::timeout_add_local_once(std::time::Duration::from_millis(32), move || {
                pending.set(false);
                da_idle.queue_draw();
            });
        });
    }

    /// Returns the underlying `DrawingArea` widget.
    #[must_use]
    #[allow(dead_code)]
    pub fn drawing_area(&self) -> &DrawingArea {
        &self.drawing_area
    }

    /// Triggers a manual redraw of the overlay.
    #[allow(dead_code)]
    pub fn queue_redraw(&self) {
        self.drawing_area.queue_draw();
    }
}
