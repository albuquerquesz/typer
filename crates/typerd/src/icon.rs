//! Tray icon drawn in code: a white keyboard glyph that matches symbolic panel
//! icons regardless of the icon theme. Off state is dimmed with a slash.

/// SNI pixmap: (width, height, ARGB32 in network byte order)
pub type Pixmap = (i32, i32, Vec<u8>);

/// Sizes offered to the tray host; it picks the closest match.
const SIZES: [usize; 3] = [22, 44, 64];
const SUPERSAMPLE: usize = 4;

pub fn pixmaps(enabled: bool) -> Vec<Pixmap> {
    SIZES.iter().map(|&size| render(size, enabled)).collect()
}

fn render(size: usize, enabled: bool) -> Pixmap {
    let scale = 64.0 / size as f32;
    let step = 1.0 / SUPERSAMPLE as f32;
    let max_alpha = if enabled { 255.0 } else { 140.0 };
    let mut data = Vec::with_capacity(size * size * 4);
    for py in 0..size {
        for px in 0..size {
            let mut hits = 0;
            for sy in 0..SUPERSAMPLE {
                for sx in 0..SUPERSAMPLE {
                    let x = (px as f32 + (sx as f32 + 0.5) * step) * scale;
                    let y = (py as f32 + (sy as f32 + 0.5) * step) * scale;
                    if covered(x, y, enabled) {
                        hits += 1;
                    }
                }
            }
            let alpha = (hits as f32 / (SUPERSAMPLE * SUPERSAMPLE) as f32 * max_alpha) as u8;
            data.extend_from_slice(&[alpha, 255, 255, 255]);
        }
    }
    (size as i32, size as i32, data)
}

/// Shapes laid out on a 64×64 canvas.
fn covered(x: f32, y: f32, enabled: bool) -> bool {
    let outline = rounded_rect(x, y, 4.0, 14.0, 60.0, 50.0, 7.0) && !rounded_rect(x, y, 8.0, 18.0, 56.0, 46.0, 3.5);
    let key_rows = [21.5, 29.5].iter().any(|&top| {
        (0..6).any(|i| {
            let left = 12.0 + i as f32 * 7.5;
            rounded_rect(x, y, left, top, left + 4.5, top + 4.5, 1.0)
        })
    });
    let spacebar = rounded_rect(x, y, 20.0, 38.0, 44.0, 42.0, 1.0);
    let slash = !enabled && segment_distance(x, y, (10.0, 6.0), (54.0, 58.0)) < 3.0;
    outline || key_rows || spacebar || slash
}

fn rounded_rect(x: f32, y: f32, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> bool {
    let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let qx = (x - cx).abs() - ((x1 - x0) / 2.0 - r);
    let qy = (y - cy).abs() - ((y1 - y0) / 2.0 - r);
    let outside = qx.max(0.0).hypot(qy.max(0.0));
    outside + qx.max(qy).min(0.0) - r <= 0.0
}

fn segment_distance(x: f32, y: f32, a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let t = (((x - a.0) * dx + (y - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    (x - (a.0 + t * dx)).hypot(y - (a.1 + t * dy))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_and_layout() {
        for (w, h, data) in pixmaps(true) {
            assert_eq!(data.len(), (w * h * 4) as usize);
        }
    }

    #[test]
    fn off_state_is_dimmer() {
        let max_alpha = |enabled| pixmaps(enabled)[2].2.chunks(4).map(|p| p[0]).max().unwrap();
        assert_eq!(max_alpha(true), 255);
        assert!(max_alpha(false) < 255);
    }
}
