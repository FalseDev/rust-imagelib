use std::{fs, io::Cursor};

use conv::ValueInto;
use image::{
    imageops, io::Reader, DynamicImage, GenericImage, GenericImageView, ImageOutputFormat, Pixel,
    Rgb, RgbImage, Rgba,
};
use imageproc::{definitions::Clamp, drawing::draw_text_mut};
use rusttype::{point, Font, Scale};

pub mod errors;
use crate::errors::Errors;

pub struct ImageOperator {
    pub image: DynamicImage,
    pub operations: Vec<ImageOperation>,
}

impl ImageOperator {
    pub fn apply_all_operations(self) -> Result<DynamicImage, Errors> {
        let mut image = self.image;
        for op in self.operations.into_iter() {
            image = op.apply(image)?;
        }
        Ok(image)
    }
}

pub enum ImageOperation {
    Overlay {
        layer_image: DynamicImage,
        coords: (i64, i64),
    },
    DrawWrappedText {
        text: String,
        color: [u8; 4],
        font: Font<'static>,
        scale: Scale,
        mid: (i32, i32),
        max_width: usize,
    },
    DrawText {
        text: String,
        color: [u8; 4],
        font: Font<'static>,
        scale: Scale,
        mid: (i32, i32),
    },
    FlipHorizontal,
    FlipVertical,
    Blur {
        sigma: f32,
    },
    ColorBlend {
        r: u8,
        g: u8,
        b: u8,
    },
}

impl ImageOperation {
    fn apply(self, mut image: DynamicImage) -> Result<DynamicImage, Errors> {
        match self {
            Self::Overlay {
                layer_image,
                coords,
            } => {
                imageops::overlay(&mut image, &layer_image, coords.0, coords.1);
                Ok(image)
            }
            Self::DrawWrappedText {
                text,
                color,
                font,
                scale,
                mid,
                max_width,
            } => {
                let color = Rgba(color);
                let final_text: &String;
                let tmp_text: String;

                if !text.contains('\n') {
                    tmp_text = textwrap::fill(&text, max_width);
                    final_text = &tmp_text;
                } else {
                    final_text = &text;
                }
                draw_text(&mut image, color, &font, final_text, scale, &mid);
                Ok(image)
            }
            Self::DrawText {
                text,
                color,
                font,
                scale,
                mid,
            } => {
                let color = Rgba(color);
                draw_text(&mut image, color, &font, &text, scale, &mid);
                Ok(image)
            }
            Self::ColorBlend { r, g, b } => {
                let color = [r, g, b];
                // let color = Rgba([r, g, b, a]);
                let h = image.height();
                let w = image.width();

                (0..w).for_each(|x| {
                    (0..h).for_each(|y| {
                        let mut pixel = image.get_pixel(x, y);
                        (0..3).for_each(|i| {
                            pixel[i] = pixel[i] / 2 + color[i] / 2;
                        });
                        image.put_pixel(x, y, pixel);
                    })
                });
                Ok(image)
            }
            Self::FlipHorizontal => Ok(image.fliph()),
            Self::FlipVertical => Ok(image.flipv()),
            Self::Blur { sigma } => Ok(image.blur(sigma)),
        }
    }
}

#[inline]
pub fn load_file(name: &str) -> Result<Vec<u8>, Errors> {
    Ok(fs::read(name)?.to_vec())
}

pub fn load_image_from_file(name: &str) -> Result<DynamicImage, Errors> {
    let v = load_file(name)?;
    let c = Cursor::new(v);
    let img = Reader::new(c).with_guessed_format()?.decode()?;
    Ok(img)
}

pub fn load_font_from_file(name: &str) -> Result<Font<'static>, Errors> {
    let font = Font::try_from_vec(fs::read(name)?.to_vec()).expect("Invalid font");
    Ok(font)
}

pub fn fill_color(color: [u8; 3], size: u32) -> RgbImage {
    let mut img = RgbImage::new(size, size);

    for x in 0..size {
        for y in 0..size {
            img.put_pixel(x, y, Rgb(color));
        }
    }
    img
}

fn get_font_height(font: &Font, scale: Scale) -> f32 {
    let v_metrics = font.v_metrics(scale);
    v_metrics.ascent - v_metrics.descent + v_metrics.line_gap
}

pub fn draw_text<'a, C>(
    image: &'a mut C,
    color: C::Pixel,
    font: &Font,
    fulltext: &str,
    scale: Scale,
    mid: &(i32, i32),
) where
    C: imageproc::drawing::Canvas,
    <C::Pixel as Pixel>::Subpixel: ValueInto<f32> + Clamp<f32>,
{
    let (raw_x, raw_y) = mid;
    let text_height = get_font_height(font, scale);
    let line_count = fulltext.lines().count() as u32;

    for (index, text) in fulltext.lines().enumerate() {
        if text.is_empty() {
            continue;
        }

        let text_width = measure_line_width(font, text, scale);
        let x = *raw_x - (text_width as i32) / 2;
        let y_delta = ((index as f32 - (line_count - 1) as f32 / 2f32) * text_height) as i32;
        let y = (*raw_y as i32 + y_delta) as i32;

        draw_text_mut(image, color, x, y, scale, font, text);
    }
}

pub fn measure_line_width(font: &Font, text: &str, scale: Scale) -> f32 {
    let width = font
        .layout(text, scale, point(0.0, 0.0))
        .map(|g| g.position().x + g.unpositioned().h_metrics().advance_width)
        .last()
        .unwrap_or(0.0);

    width
}

pub fn image_to_bytes(image: DynamicImage, format: ImageOutputFormat) -> Result<Vec<u8>, Errors> {
    let mut bytes: Vec<u8> = Vec::new();
    let mut w = Cursor::new(&mut bytes);
    image.write_to(&mut w, format)?;
    Ok(bytes)
}
