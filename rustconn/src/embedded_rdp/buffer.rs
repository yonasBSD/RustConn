//! Pixel buffer and Wayland surface handling for embedded RDP
//!
//! This module contains the `PixelBuffer` struct for frame data storage,
//! `CairoBackedBuffer` for zero-copy rendering, and `WaylandSurfaceHandle`
//! for Wayland subsurface integration.

use super::types::EmbeddedRdpError;

/// Pixel buffer for frame data
///
/// This struct holds the pixel data received from FreeRDP's EndPaint callback
/// and is used to blit to the Wayland surface.
#[derive(Debug)]
pub struct PixelBuffer {
    /// Raw pixel data in BGRA format
    data: Vec<u8>,
    /// Buffer width in pixels
    width: u32,
    /// Buffer height in pixels
    height: u32,
    /// Stride (bytes per row)
    stride: u32,
    /// Whether the buffer has received any data
    has_data: bool,
}

impl PixelBuffer {
    /// Creates a new pixel buffer with the specified dimensions
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        let stride = width * 4; // BGRA = 4 bytes per pixel
        let size = (stride * height) as usize;
        Self {
            data: vec![0; size],
            width,
            height,
            stride,
            has_data: false,
        }
    }

    /// Returns the buffer width
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Returns the buffer height
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Returns whether the buffer has received any data
    #[must_use]
    pub const fn has_data(&self) -> bool {
        self.has_data
    }

    /// Sets the has_data flag
    pub fn set_has_data(&mut self, has_data: bool) {
        self.has_data = has_data;
    }

    /// Returns the stride (bytes per row)
    #[must_use]
    pub const fn stride(&self) -> u32 {
        self.stride
    }

    /// Returns a reference to the raw pixel data
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// Returns a mutable reference to the raw pixel data
    pub fn data_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    /// Resizes the buffer to new dimensions
    ///
    /// Preserves existing content by scaling it to the new size to avoid
    /// visual artifacts during resize. The has_data flag is preserved.
    pub fn resize(&mut self, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return; // No change needed
        }

        let old_width = self.width;
        let old_height = self.height;
        let had_data = self.has_data;

        self.width = width;
        self.height = height;
        self.stride = width * 4;
        let new_size = (self.stride * height) as usize;

        if had_data && old_width > 0 && old_height > 0 {
            // Preserve old data - just resize the buffer
            // The old content will be scaled during rendering
            self.data.resize(new_size, 0);
            self.has_data = true; // Keep has_data true to continue rendering
        } else {
            self.data.resize(new_size, 0);
            self.has_data = false;
        }
    }

    /// Clears the buffer to black
    pub fn clear(&mut self) {
        self.data.fill(0);
        self.has_data = false; // Reset data flag on clear
    }

    /// Updates a region of the buffer
    ///
    /// # Arguments
    ///
    /// * `x` - X coordinate of the region
    /// * `y` - Y coordinate of the region
    /// * `w` - Width of the region
    /// * `h` - Height of the region
    /// * `src_data` - Source pixel data
    /// * `src_stride` - Source stride
    pub fn update_region(
        &mut self,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        src_data: &[u8],
        src_stride: u32,
    ) {
        let dst_stride = self.stride as usize;
        let src_stride = src_stride as usize;
        let bytes_per_pixel = 4;

        for row in 0..h {
            let dst_y = (y + row) as usize;
            if dst_y >= self.height as usize {
                break;
            }

            let x_offset = x as usize * bytes_per_pixel;
            if x_offset >= dst_stride {
                continue;
            }

            let dst_offset = dst_y * dst_stride + x_offset;
            let src_offset = row as usize * src_stride;
            let max_copy = dst_stride.saturating_sub(x_offset);
            let copy_width = (w as usize * bytes_per_pixel).min(max_copy);

            if copy_width > 0
                && src_offset + copy_width <= src_data.len()
                && dst_offset + copy_width <= self.data.len()
            {
                self.data[dst_offset..dst_offset + copy_width]
                    .copy_from_slice(&src_data[src_offset..src_offset + copy_width]);
                self.has_data = true; // Mark that we have received data
            }
        }
    }
}

/// A pixel buffer backed by a persistent Cairo `ImageSurface`.
///
/// Instead of cloning 33MB of pixel data on every draw call (at 4K),
/// this struct owns the underlying byte buffer via Cairo's
/// `ImageSurface::create_for_data()` and provides mutable access
/// through `surface.data()` for in-place updates.
///
/// The Cairo surface is created once and reused across frames.
/// Only `surface.mark_dirty_rectangle()` is needed to tell Cairo
/// which regions changed.
pub struct CairoBackedBuffer {
    surface: Option<gtk4::cairo::ImageSurface>,
    width: u32,
    height: u32,
    stride: u32,
    has_data: bool,
}

impl CairoBackedBuffer {
    /// Creates a new Cairo-backed buffer with the specified dimensions.
    ///
    /// The underlying `ImageSurface` is created lazily on first use
    /// via `ensure_surface()`.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        let stride = width * 4;
        let mut buf = Self {
            surface: None,
            width,
            height,
            stride,
            has_data: false,
        };
        buf.ensure_surface();
        buf
    }

    /// Lazily creates the Cairo `ImageSurface` if it doesn't exist yet.
    fn ensure_surface(&mut self) {
        if self.surface.is_some() || self.width == 0 || self.height == 0 {
            return;
        }
        let size = (self.stride * self.height) as usize;
        let data = vec![0u8; size];
        match gtk4::cairo::ImageSurface::create_for_data(
            data,
            gtk4::cairo::Format::ARgb32,
            crate::utils::dimension_to_i32(self.width),
            crate::utils::dimension_to_i32(self.height),
            crate::utils::stride_to_i32(self.stride),
        ) {
            Ok(s) => {
                self.surface = Some(s);
            }
            Err(e) => {
                tracing::warn!("Failed to create Cairo surface: {e}");
            }
        }
    }

    /// Returns the buffer width.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Returns the buffer height.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Returns the stride (bytes per row).
    #[must_use]
    pub const fn stride(&self) -> u32 {
        self.stride
    }

    /// Returns whether the buffer has received any frame data.
    #[must_use]
    pub const fn has_data(&self) -> bool {
        self.has_data
    }

    /// Returns a reference to the underlying `ImageSurface`, if available.
    #[must_use]
    pub fn surface(&self) -> Option<&gtk4::cairo::ImageSurface> {
        self.surface.as_ref()
    }

    /// Updates a rectangular region of the surface's pixel data in-place.
    ///
    /// After writing, calls `mark_dirty_rectangle` so Cairo knows which
    /// area needs to be re-composited.
    #[allow(clippy::many_single_char_names)]
    pub fn update_region(
        &mut self,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
        src_data: &[u8],
        src_stride: u32,
    ) {
        let Some(ref mut surface) = self.surface else {
            return;
        };

        // Get mutable access to the surface's pixel data
        let mut data = match surface.data() {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!("Failed to lock surface data: {e}");
                return;
            }
        };

        let dst_stride = self.stride as usize;
        let src_stride_usize = src_stride as usize;
        let bpp = 4;

        for row in 0..h {
            let dst_y = (y + row) as usize;
            if dst_y >= self.height as usize {
                break;
            }

            let x_off = x as usize * bpp;
            if x_off >= dst_stride {
                continue;
            }

            let dst_off = dst_y * dst_stride + x_off;
            let src_off = row as usize * src_stride_usize;
            let copy_w = (w as usize * bpp).min(dst_stride - x_off);

            if copy_w > 0 && src_off + copy_w <= src_data.len() && dst_off + copy_w <= data.len() {
                data[dst_off..dst_off + copy_w]
                    .copy_from_slice(&src_data[src_off..src_off + copy_w]);
            }
        }

        // Drop the data borrow before marking dirty
        drop(data);

        // Tell Cairo which rectangle changed
        surface.mark_dirty_rectangle(x as i32, y as i32, w as i32, h as i32);

        self.has_data = true;
    }

    /// Recreates the surface when dimensions change.
    ///
    /// The old surface is dropped and a new one is allocated.
    pub fn resize(&mut self, width: u32, height: u32) {
        if self.width == width && self.height == height {
            return;
        }
        self.width = width;
        self.height = height;
        self.stride = width * 4;
        self.has_data = false;
        self.surface = None;
        self.ensure_surface();
    }

    /// Clears the buffer to black (zeros) and marks the entire surface dirty.
    pub fn clear(&mut self) {
        if let Some(ref mut surface) = self.surface {
            {
                // Scope the mutable data borrow so it's dropped before mark_dirty
                if let Ok(mut data) = surface.data() {
                    data.fill(0);
                }
            }
            surface.mark_dirty();
        }
        self.has_data = false;
    }

    /// Fills the buffer with a solid colour (used for resize placeholder).
    ///
    /// Each pixel is written as `[b, g, r, a]` in BGRA order.
    pub fn fill_solid(&mut self, b: u8, g: u8, r: u8, a: u8) {
        if let Some(ref mut surface) = self.surface {
            {
                // Scope the mutable data borrow so it's dropped before mark_dirty
                if let Ok(mut data) = surface.data() {
                    for chunk in data.chunks_exact_mut(4) {
                        chunk[0] = b;
                        chunk[1] = g;
                        chunk[2] = r;
                        chunk[3] = a;
                    }
                }
            }
            surface.mark_dirty();
        }
        self.has_data = true;
    }
}

/// Wayland surface handle for subsurface integration
///
/// This struct manages the Wayland surface resources for embedding
/// the RDP display within the GTK widget hierarchy.
#[derive(Debug, Default)]
pub struct WaylandSurfaceHandle {
    /// Whether the surface is initialized
    initialized: bool,
    /// Surface ID (for debugging)
    surface_id: u32,
}

impl WaylandSurfaceHandle {
    /// Creates a new uninitialized surface handle
    #[must_use]
    pub const fn new() -> Self {
        Self {
            initialized: false,
            surface_id: 0,
        }
    }

    /// Initializes the Wayland surface
    ///
    /// # Errors
    ///
    /// Returns error if surface creation fails
    pub fn initialize(&mut self) -> Result<(), EmbeddedRdpError> {
        // In a real implementation, this would:
        // 1. Get the wl_display from GTK
        // 2. Create a wl_surface
        // 3. Create a wl_subsurface attached to the parent
        // 4. Set up shared memory buffers

        // For now, we mark as initialized for the fallback path
        self.initialized = true;
        self.surface_id = 1;
        Ok(())
    }

    /// Returns whether the surface is initialized
    #[must_use]
    pub const fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Commits pending changes to the surface
    pub fn commit(&self) {
        // In a real implementation, this would call wl_surface_commit
    }

    /// Damages a region of the surface for redraw
    pub fn damage(&self, _x: i32, _y: i32, _width: i32, _height: i32) {
        // In a real implementation, this would call wl_surface_damage_buffer
    }

    /// Cleans up the surface resources
    pub fn cleanup(&mut self) {
        self.initialized = false;
        self.surface_id = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixel_buffer_new() {
        let buffer = PixelBuffer::new(100, 50);
        assert_eq!(buffer.width(), 100);
        assert_eq!(buffer.height(), 50);
        assert_eq!(buffer.stride(), 400); // 100 * 4 bytes per pixel
        assert_eq!(buffer.data().len(), 20000); // 100 * 50 * 4
    }

    #[test]
    fn test_pixel_buffer_resize() {
        let mut buffer = PixelBuffer::new(100, 50);
        buffer.resize(200, 100);
        assert_eq!(buffer.width(), 200);
        assert_eq!(buffer.height(), 100);
        assert_eq!(buffer.stride(), 800);
        assert_eq!(buffer.data().len(), 80000);
    }

    #[test]
    fn test_pixel_buffer_clear() {
        let mut buffer = PixelBuffer::new(10, 10);
        buffer.data_mut()[0] = 255;
        buffer.clear();
        assert!(buffer.data().iter().all(|&b| b == 0));
    }

    #[test]
    fn test_wayland_surface_handle() {
        let mut handle = WaylandSurfaceHandle::new();
        assert!(!handle.is_initialized());

        handle.initialize().unwrap();
        assert!(handle.is_initialized());

        handle.cleanup();
        assert!(!handle.is_initialized());
    }
}
