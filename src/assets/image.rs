use std::error::Error;
use std::path::Path;

use druid::{
    piet::{ImageFormat, InterpolationMode},
    Affine, PaintCtx, RenderContext, Size,
};
use image::{self, DynamicImage, Pixel, RgbaImage};
use palette::{Hsva, RgbHue, Srgba};

use super::AssetStore;

fn map_image_hsv<F>(img: &mut RgbaImage, f: F)
where
    F: Fn(&mut Hsva),
{
    for p in img.pixels_mut() {
        let channels = p.channels_mut();
        let mut hsv_color: Hsva = Srgba::new(
            channels[0] as f32 / 255.0,
            channels[1] as f32 / 255.0,
            channels[2] as f32 / 255.0,
            channels[3] as f32 / 255.0,
        )
        .into();
        f(&mut hsv_color);
        let rgb_color: Srgba = hsv_color.into();
        channels[0] = (rgb_color.color.red * 255.0) as u8;
        channels[1] = (rgb_color.color.green * 255.0) as u8;
        channels[2] = (rgb_color.color.blue * 255.0) as u8;
        channels[3] = (rgb_color.alpha * 255.0) as u8;
    }
}
fn make_disabled_image(src: &DynamicImage) -> DynamicImage {
    let mut img = src.clone().to_rgba();
    map_image_hsv(&mut img, |hsv| {
        hsv.saturation = 0.0;
        hsv.value *= 0.1;
    });

    DynamicImage::ImageRgba8(img)
}

fn make_completed_image(src: &DynamicImage) -> DynamicImage {
    let mut img = src.clone().to_rgba();
    map_image_hsv(&mut img, |hsv| {
        hsv.hue = RgbHue::from_degrees(120.0);
        hsv.value *= 0.35;
    });

    DynamicImage::ImageRgba8(img)
}

pub(crate) fn add_image_to_cache(store: &mut AssetStore<ImageData>, id: &str, data: &[u8]) {
    let image = image::load_from_memory(data).unwrap();
    store.add(&id.to_string(), ImageData::from_dynamic_image(image));
}

pub(crate) fn add_objective_to_cache(store: &mut AssetStore<ImageData>, id: &str, data: &[u8]) {
    let image = image::load_from_memory(data).unwrap();
    let disabled_image = make_disabled_image(&image);
    let completed_image = make_completed_image(&image);
    store.add(&id.to_string(), ImageData::from_dynamic_image(image));
    store.add(
        &format!("{}:disabled", id),
        ImageData::from_dynamic_image(disabled_image),
    );
    store.add(
        &format!("{}:completed", id),
        ImageData::from_dynamic_image(completed_image),
    );
}

/// Stored Image data.
#[derive(Clone)]
pub struct ImageData {
    pixels: Vec<u8>,
    x_pixels: u32,
    y_pixels: u32,
    format: ImageFormat,
}

impl ImageData {
    /// Create an empty Image
    pub fn empty() -> Self {
        ImageData {
            pixels: [].to_vec(),
            x_pixels: 0,
            y_pixels: 0,
            format: ImageFormat::RgbaSeparate,
        }
    }

    /// Load an image from a DynamicImage from the image crate
    pub fn from_dynamic_image(image_data: image::DynamicImage) -> ImageData {
        if has_alpha_channel(&image_data) {
            Self::from_dynamic_image_with_alpha(image_data)
        } else {
            Self::from_dynamic_image_without_alpha(image_data)
        }
    }

    /// Load an image from a DynamicImage with alpha
    pub fn from_dynamic_image_with_alpha(image_data: image::DynamicImage) -> ImageData {
        let rgba_image = image_data.to_rgba();
        let sizeofimage = rgba_image.dimensions();
        ImageData {
            pixels: rgba_image.to_vec(),
            x_pixels: sizeofimage.0,
            y_pixels: sizeofimage.1,
            format: ImageFormat::RgbaSeparate,
        }
    }

    /// Load an image from a DynamicImage without alpha
    pub fn from_dynamic_image_without_alpha(image_data: image::DynamicImage) -> ImageData {
        let rgb_image = image_data.to_rgb();
        let sizeofimage = rgb_image.dimensions();
        ImageData {
            pixels: rgb_image.to_vec(),
            x_pixels: sizeofimage.0,
            y_pixels: sizeofimage.1,
            format: ImageFormat::Rgb,
        }
    }

    /// Attempt to load an image from raw bytes.
    ///
    /// If the image crate can't decode an image from the data an error will be returned.
    #[allow(dead_code)]
    pub fn from_data(raw_image: &[u8]) -> Result<Self, Box<dyn Error>> {
        let image_data = image::load_from_memory(raw_image).map_err(|e| e)?;
        Ok(ImageData::from_dynamic_image(image_data))
    }

    /// Attempt to load an image from the file at the provided path.
    #[allow(dead_code)]
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn Error>> {
        let image_data = image::open(path).map_err(|e| e)?;
        Ok(ImageData::from_dynamic_image(image_data))
    }

    /// Get the size in pixels of the contained image.
    pub fn get_size(&self) -> Size {
        Size::new(self.x_pixels as f64, self.y_pixels as f64)
    }

    /// Convert ImageData into Piet draw instructions.
    pub fn to_piet(
        &self,
        offset_matrix: Affine,
        ctx: &mut PaintCtx,
        interpolation: InterpolationMode,
    ) {
        ctx.with_save(|ctx| {
            ctx.transform(offset_matrix);
            let size = self.get_size();
            let im = ctx
                .make_image(
                    size.width as usize,
                    size.height as usize,
                    &self.pixels,
                    self.format,
                )
                .unwrap();
            ctx.draw_image(&im, size.to_rect(), interpolation);
        })
    }
}

fn has_alpha_channel(image: &image::DynamicImage) -> bool {
    use image::ColorType::*;
    match image.color() {
        La8 | Rgba8 | La16 | Rgba16 | Bgra8 => true,
        _ => false,
    }
}

impl Default for ImageData {
    fn default() -> Self {
        ImageData::empty()
    }
}
