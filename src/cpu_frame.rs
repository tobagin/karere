//! Damage-sized, immutable CPU textures. A chat-list repaint must not upload
//! the unchanged conversation too. One-pixel borders preserve linear sampling.

pub type Rect = (i32, i32, i32, i32);

pub struct Tile {
    pub rect: Rect,
    pub sample: Rect,
    pub bytes: glib::Bytes,
    pub dirty: bool,
}

#[derive(Default)]
pub struct Frame {
    pub tiles: Vec<Tile>,
    size: (i32, i32),
    edge: i32,
}

impl Frame {
    pub fn update(
        &mut self,
        source: &[u8],
        width: i32,
        height: i32,
        scale: f32,
        damage: Option<Rect>,
    ) {
        // Multiples of 120 logical pixels align with Wayland's fractional-scale
        // protocol. Keep tile boundaries on device pixels at steady geometry.
        let edge = (240.0 * scale.max(1.0)).round() as i32;
        let resized = self.size != (width, height) || self.edge != edge;
        if resized {
            self.tiles.clear();
            self.size = (width, height);
            self.edge = edge;
            for y in (0..height).step_by(edge as usize) {
                for x in (0..width).step_by(edge as usize) {
                    let w = edge.min(width - x);
                    let h = edge.min(height - y);
                    let sx = (x - 1).max(0);
                    let sy = (y - 1).max(0);
                    self.tiles.push(Tile {
                        rect: (x, y, w, h),
                        sample: (
                            sx,
                            sy,
                            (x + w + 1).min(width) - sx,
                            (y + h + 1).min(height) - sy,
                        ),
                        bytes: glib::Bytes::from_static(&[]),
                        dirty: false,
                    });
                }
            }
        }
        for tile in &mut self.tiles {
            if !resized && damage.is_some_and(|damage| !intersects(tile.sample, damage)) {
                continue;
            }
            let (x, y, w, h) = tile.sample;
            let row_bytes = w as usize * 4;
            let mut bytes = Vec::with_capacity(row_bytes * h as usize);
            for row in y..y + h {
                let start = (row as usize * width as usize + x as usize) * 4;
                bytes.extend_from_slice(&source[start..start + row_bytes]);
            }
            tile.bytes = glib::Bytes::from_owned(bytes);
            tile.dirty = true;
        }
    }
}

fn intersects(a: Rect, b: Rect) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_keeps_unchanged_tiles_and_updates_neighbour_sampling_borders() {
        let mut frame = Frame::default();
        let mut pixels = vec![17; 500 * 4 * 4];
        frame.update(&pixels, 500, 4, 1.0, None);
        let unchanged = frame.tiles[2].bytes.clone();
        let old = frame.tiles[0].bytes.clone();
        for tile in &mut frame.tiles {
            tile.dirty = false;
        }
        pixels[239 * 4..240 * 4].fill(29);
        frame.update(&pixels, 500, 4, 1.0, Some((239, 0, 1, 1)));
        assert!(frame.tiles[0].dirty && frame.tiles[1].dirty);
        assert!(!frame.tiles[2].dirty);
        assert_eq!(unchanged.as_ptr(), frame.tiles[2].bytes.as_ptr());
        assert_eq!(
            old[239 * 4],
            17,
            "an already-presented texture is immutable"
        );
        assert_eq!(frame.tiles[1].bytes[0], 29, "neighbour gutter must match");
        frame.update(&[7; 8], 2, 1, 1.0, Some((0, 0, 1, 1)));
        assert_eq!(frame.tiles.len(), 1);
        assert_eq!(&frame.tiles[0].bytes[..], &[7; 8]);
    }
}
