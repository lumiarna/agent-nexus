//! Windows-taskbar tray icons for `Provider quota`.
//!
//! Each tray-visible `Provider` gets its own icon (a `Surface Preference`,
//! independent of the Provider card): a rounded square in the provider's brand
//! colour with the "shortest window used" number rendered on top. The front end
//! owns branding and the metric口径 (Used / Remaining); it pushes the desired
//! icon set here via `sync_tray`, and this module reconciles the live tray to
//! match — creating, updating, and removing icons.
//!
//! All tray mutations are marshalled onto the main (GUI) thread, since Windows
//! tray/menu APIs are not safe to touch from a command's worker thread.

use std::collections::HashMap;
use std::sync::Mutex;

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use serde::Deserialize;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, PremultipliedColorU8, Transform};

/// Roboto Bold (Apache-2.0). Only the digits are ever rendered.
const FONT: &[u8] = include_bytes!("../assets/tray-digits.ttf");

const ICON_SIZE: u32 = 32;
const MENU_SHOW_ID: &str = "tray-show";
const MENU_QUIT_ID: &str = "tray-quit";

/// One desired tray icon, computed by the front end from live quota.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayEntry {
    pub provider_id: String,
    /// Display name for the tooltip (e.g. "Claude Code").
    pub label: String,
    /// Brand colour as `#rrggbb`.
    pub color_hex: String,
    /// The number to render, already resolved to the Used/Remaining口径 (0–100).
    /// `None` means the quota fetch failed and should render a failure marker.
    pub value: Option<i64>,
}

/// Live tray icons keyed by provider id. Managed as Tauri state.
#[derive(Default)]
pub struct TrayManager {
    icons: Mutex<HashMap<String, TrayIcon>>,
}

impl TrayManager {
    /// Whether any tray icon is currently live. The window's close handler uses
    /// this to decide between hide-to-tray and exit: hiding with no icon would
    /// orphan the process with no way to restore the window.
    pub fn has_icons(&self) -> bool {
        self.icons
            .lock()
            .map(|icons| !icons.is_empty())
            .unwrap_or(false)
    }
}

/// Reconcile the live tray to `entries`. Returns immediately; the actual work
/// runs on the main thread.
pub fn sync_tray(app: &AppHandle, entries: Vec<TrayEntry>) -> tauri::Result<()> {
    let handle = app.clone();
    app.run_on_main_thread(move || {
        if let Err(error) = reconcile(&handle, entries) {
            eprintln!("tray sync failed: {error}");
        }
    })
}

type TrayResult<T> = Result<T, Box<dyn std::error::Error>>;

fn reconcile(app: &AppHandle, entries: Vec<TrayEntry>) -> TrayResult<()> {
    let manager = app.state::<TrayManager>();
    let mut icons = manager.icons.lock().expect("tray icon map poisoned");

    let stale: Vec<String> = icons
        .keys()
        .filter(|id| !entries.iter().any(|e| &e.provider_id == *id))
        .cloned()
        .collect();
    for id in stale {
        icons.remove(&id);
        app.remove_tray_by_id(tray_id(&id).as_str());
    }

    for entry in entries {
        let label_value = entry
            .value
            .map(|value| value.clamp(0, 100).to_string())
            .unwrap_or_else(|| "-".to_string());
        let image = render_icon(&entry.color_hex, &label_value)?;
        let tooltip = if entry.value.is_some() {
            format!("{}: {label_value}%", entry.label)
        } else {
            format!("{}: -", entry.label)
        };

        if let Some(icon) = icons.get(&entry.provider_id) {
            icon.set_icon(Some(image))?;
            icon.set_tooltip(Some(&tooltip))?;
        } else {
            let menu = build_menu(app)?;
            let icon = TrayIconBuilder::with_id(tray_id(&entry.provider_id))
                .icon(image)
                .tooltip(&tooltip)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    MENU_SHOW_ID => show_main_window(app),
                    MENU_QUIT_ID => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main_window(tray.app_handle());
                    }
                })
                .build(app)?;
            icons.insert(entry.provider_id, icon);
        }
    }

    Ok(())
}

fn tray_id(provider_id: &str) -> String {
    format!("provider-quota-{provider_id}")
}

fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let show = MenuItem::with_id(app, MENU_SHOW_ID, "Show Agent Nexus", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT_ID, "Quit", true, None::<&str>)?;
    Menu::with_items(app, &[&show, &quit])
}

/// Show and focus the main window (used from tray click and the Show menu item).
pub fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Render a `ICON_SIZE`×`ICON_SIZE` RGBA tray icon: a rounded square in the
/// brand colour with `value` drawn in white, auto-sized to fit.
fn render_icon(color_hex: &str, text: &str) -> TrayResult<Image<'static>> {
    let rgba = render_rgba(color_hex, text)?;
    Ok(Image::new_owned(rgba, ICON_SIZE, ICON_SIZE))
}

/// The un-premultiplied RGBA bytes behind [`render_icon`], split out so the
/// rasterisation can be unit-tested without a running app.
fn render_rgba(color_hex: &str, text: &str) -> TrayResult<Vec<u8>> {
    let pixmap = render_pixmap(color_hex, text)?;
    Ok(pixmap
        .pixels()
        .iter()
        .flat_map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect())
}

fn render_pixmap(color_hex: &str, text: &str) -> TrayResult<Pixmap> {
    let (r, g, b) =
        parse_hex(color_hex).ok_or_else(|| format!("invalid tray brand colour: {color_hex:?}"))?;
    let s = ICON_SIZE as f32;

    let mut pixmap = Pixmap::new(ICON_SIZE, ICON_SIZE).ok_or("failed to allocate tray pixmap")?;

    let background = rounded_rect(0.5, 0.5, s - 1.0, s - 1.0, 7.0);
    let mut paint = Paint::default();
    paint.set_color_rgba8(r, g, b, 255);
    paint.anti_alias = true;
    pixmap.fill_path(
        &background,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );

    draw_centered_text(&mut pixmap, text);
    Ok(pixmap)
}

fn draw_centered_text(pixmap: &mut Pixmap, text: &str) {
    let font = FontRef::try_from_slice(FONT).expect("bundled tray font is valid");
    let s = ICON_SIZE as f32;
    let max_w = s * 0.90;
    let max_h = s * 0.62;

    // Pass 1: measure real glyph bounds at a reference scale, then derive the
    // scale that fits width and height (no magic cap-height constant).
    let (w0, h0) = measure(&font, text, s);
    if w0 <= 0.0 || h0 <= 0.0 {
        return;
    }
    let px = s * (max_w / w0).min(max_h / h0);

    // Pass 2: lay out at the final scale and centre using the real bounds.
    let scale = PxScale::from(px);
    let scaled = font.as_scaled(scale);
    let mut pen_x = 0.0f32;
    let mut glyphs = Vec::new();
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    for ch in text.chars() {
        let gid = font.glyph_id(ch);
        let glyph = gid.with_scale_and_position(scale, ab_glyph::point(pen_x, 0.0));
        if let Some(outline) = font.outline_glyph(glyph) {
            let b = outline.px_bounds();
            min_x = min_x.min(b.min.x);
            min_y = min_y.min(b.min.y);
            max_x = max_x.max(b.max.x);
            max_y = max_y.max(b.max.y);
            glyphs.push(outline);
        }
        pen_x += scaled.h_advance(gid);
    }
    if glyphs.is_empty() {
        return;
    }

    let off_x = (s - (max_x - min_x)) / 2.0 - min_x;
    let off_y = (s - (max_y - min_y)) / 2.0 - min_y;
    let width = ICON_SIZE as i32;

    for outline in &glyphs {
        let bounds = outline.px_bounds();
        outline.draw(|gx, gy, coverage| {
            let x = (bounds.min.x + gx as f32 + off_x).round() as i32;
            let y = (bounds.min.y + gy as f32 + off_y).round() as i32;
            if x < 0 || y < 0 || x >= width || y >= ICON_SIZE as i32 {
                return;
            }
            blend_white(pixmap, (y * width + x) as usize, coverage);
        });
    }
}

/// Union glyph bounds width/height at a given px scale.
fn measure(font: &FontRef, text: &str, px: f32) -> (f32, f32) {
    let scale = PxScale::from(px);
    let scaled = font.as_scaled(scale);
    let mut pen_x = 0.0f32;
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
    let mut any = false;
    for ch in text.chars() {
        let gid = font.glyph_id(ch);
        let glyph = gid.with_scale_and_position(scale, ab_glyph::point(pen_x, 0.0));
        if let Some(outline) = font.outline_glyph(glyph) {
            let b = outline.px_bounds();
            min_x = min_x.min(b.min.x);
            min_y = min_y.min(b.min.y);
            max_x = max_x.max(b.max.x);
            max_y = max_y.max(b.max.y);
            any = true;
        }
        pen_x += scaled.h_advance(gid);
    }
    if any {
        (max_x - min_x, max_y - min_y)
    } else {
        (0.0, 0.0)
    }
}

/// Source-over blend of opaque white at `coverage` onto a premultiplied pixel.
fn blend_white(pixmap: &mut Pixmap, index: usize, coverage: f32) {
    let pixels = pixmap.pixels_mut();
    let Some(dst) = pixels.get(index) else {
        return;
    };
    let a = (coverage.clamp(0.0, 1.0) * 255.0).round() as u16;
    let inv = 255 - a;
    let mix = |channel: u8| (a + (channel as u16 * inv) / 255).min(255) as u8;
    if let Some(color) = PremultipliedColorU8::from_rgba(
        mix(dst.red()),
        mix(dst.green()),
        mix(dst.blue()),
        mix(dst.alpha()),
    ) {
        pixels[index] = color;
    }
}

fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> tiny_skia::Path {
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    pb.finish().expect("rounded rect path is valid")
}

/// Parse `#rrggbb` (or `rrggbb`). Returns `None` on malformed input rather than
/// painting a wrong colour — the front end always sends a valid brand colour.
fn parse_hex(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    Some((
        u8::from_str_radix(&hex[0..2], 16).ok()?,
        u8::from_str_radix(&hex[2..4], 16).ok()?,
        u8::from_str_radix(&hex[4..6], 16).ok()?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_reads_rrggbb_with_or_without_hash() {
        assert_eq!(parse_hex("#c2410c"), Some((0xc2, 0x41, 0x0c)));
        assert_eq!(parse_hex("  c2410c "), Some((0xc2, 0x41, 0x0c)));
    }

    #[test]
    fn parse_hex_rejects_malformed_input() {
        assert_eq!(parse_hex("#fff"), None);
        assert_eq!(parse_hex("#zzzzzz"), None);
        assert_eq!(parse_hex(""), None);
    }

    #[test]
    fn render_rgba_errors_on_bad_colour() {
        assert!(render_rgba("not-a-colour", "42").is_err());
    }

    #[test]
    fn render_rgba_paints_brand_square_transparent_corners_and_white_text() {
        // Claude Code brand orange.
        let (br, bg, bb) = (0xc2u8, 0x41u8, 0x0cu8);
        let rgba = render_rgba("#c2410c", "42").expect("render icon");
        assert_eq!(rgba.len(), (ICON_SIZE * ICON_SIZE * 4) as usize);

        let px = |x: u32, y: u32| {
            let i = ((y * ICON_SIZE + x) * 4) as usize;
            (rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3])
        };

        // Rounded corner is fully transparent; centre is opaque.
        assert_eq!(px(0, 0).3, 0, "top-left corner must be transparent");
        assert_eq!(
            px(ICON_SIZE / 2, ICON_SIZE / 2).3,
            255,
            "centre must be opaque"
        );

        // The brand colour fills the square, and white digits are drawn on top.
        let mut brand = false;
        let mut white = false;
        for chunk in rgba.chunks_exact(4) {
            let (r, g, b, a) = (chunk[0], chunk[1], chunk[2], chunk[3]);
            if a == 255 && r == br && g == bg && b == bb {
                brand = true;
            }
            if a == 255 && r > 220 && g > 220 && b > 220 {
                white = true;
            }
        }
        assert!(brand, "brand-colour pixels must be present");
        assert!(white, "white text pixels must be present");
    }

    #[test]
    fn render_rgba_paints_failure_marker() {
        let rgba = render_rgba("#c2410c", "-").expect("render failure marker");
        assert!(rgba.chunks_exact(4).any(|chunk| {
            let (r, g, b, a) = (chunk[0], chunk[1], chunk[2], chunk[3]);
            a == 255 && r > 220 && g > 220 && b > 220
        }));
    }
}
