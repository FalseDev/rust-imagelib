use std::{fs, io::Cursor};

use conv::ValueInto;
use image::imageops::FilterType;
pub use image::{
    imageops, io::Reader, DynamicImage, GenericImage, GenericImageView, ImageOutputFormat, Pixel,
    Rgb, RgbImage, Rgba,
};
pub use imageproc::{definitions::Clamp, drawing::draw_text_mut};
pub use rusttype::{point, Font, Scale};

pub mod errors;
#[cfg(feature = "serde")]
use serde::Deserialize;

use crate::errors::Errors;

#[cfg_attr(
    feature = "serde",
    derive(Deserialize),
    serde(rename_all = "lowercase")
)]
pub struct ImageInput {
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub image_input_type: ImageInputType,
    #[cfg_attr(feature = "serde", serde(default))]
    pub operations: Vec<ImageOperation>,
}

impl ImageInput {
    pub fn get_image(self) -> Result<DynamicImage, Errors> {
        let mut image = self.image_input_type.get_image()?;
        for operation in self.operations.into_iter() {
            image = operation.apply(image)?;
        }
        Ok(image)
    }
}

#[cfg_attr(
    feature = "serde",
    derive(Deserialize),
    serde(rename_all = "lowercase")
)]
pub enum ImageInputType {
    #[cfg_attr(feature = "serde", serde(skip_deserializing))]
    DynamicImage(DynamicImage),
    Color {
        r: u8,
        g: u8,
        b: u8,
        size: (u32, u32),
    },
    Filename(String),
    Bytes(Vec<u8>),
}

impl ImageInputType {
    pub fn get_image(self) -> Result<DynamicImage, Errors> {
        match self {
            Self::DynamicImage(image) => Ok(image),
            Self::Color { r, g, b, size } => {
                Ok(DynamicImage::ImageRgb8(fill_color([r, g, b], size)))
            }
            Self::Filename(name) => load_image_from_file(&name),
            Self::Bytes(bytes) => Ok(Reader::new(Cursor::new(bytes))
                .with_guessed_format()?
                .decode()?),
        }
    }
}

#[cfg_attr(
    feature = "serde",
    derive(Deserialize),
    serde(rename_all = "lowercase")
)]
pub enum FontInput {
    #[cfg_attr(feature = "serde", serde(skip_deserializing))]
    Font(Font<'static>),
    Filename(String),
    Bytes(Vec<u8>),
}

impl FontInput {
    pub fn get_font(self) -> Result<Font<'static>, Errors> {
        match self {
            Self::Font(font) => Ok(font),
            Self::Filename(name) => load_font_from_file(&name),
            Self::Bytes(bytes) => Font::try_from_vec(bytes).ok_or(Errors::InvalidFont),
        }
    }
}

#[cfg_attr(
    feature = "serde",
    derive(Deserialize),
    serde(rename_all = "lowercase")
)]
pub struct ImageOperator {
    pub image_input: Option<ImageInput>,
    pub operations: Vec<ImageOperation>,
    #[cfg_attr(feature = "serde", serde(skip_deserializing))]
    image: Option<DynamicImage>,
}

impl ImageOperator {
    pub fn new(image_input: ImageInput, operations: Vec<ImageOperation>) -> Self {
        Self {
            image_input: Some(image_input),
            operations,
            image: None,
        }
    }

    pub fn apply_all_operations(self) -> Result<Self, Errors> {
        let mut image = self
            .image_input
            .ok_or(Errors::InputImageAlreadyUsed)?
            .get_image()?;
        for op in self.operations.into_iter() {
            image = op.apply(image)?;
        }
        Ok(Self {
            image_input: None,
            operations: Vec::new(),
            image: Some(image),
        })
    }
    pub fn get_image(self) -> Option<DynamicImage> {
        self.image
    }
}

#[cfg_attr(
    feature = "serde",
    derive(Deserialize),
    serde(rename_all = "lowercase")
)]
pub struct ScaleTuple(pub f32, pub f32);
impl ScaleTuple {
    fn to_scale(&self) -> Scale {
        Scale {
            x: self.0,
            y: self.1,
        }
    }
}

#[cfg_attr(
    feature = "serde",
    derive(Deserialize),
    serde(rename_all = "lowercase")
)]
pub enum ImageOperation {
    Resize {
        h: u32,
        w: u32,
        filter: String,
    },
    Crop {
        x: u32,
        y: u32,
        w: u32,
        h: u32,
    },
    Overlay {
        layer_image_input: ImageInput,
        coords: (i64, i64),
    },
    DrawWrappedText {
        text: String,
        color: [u8; 4],
        font: FontInput,
        scale: ScaleTuple,
        mid: (i32, i32),
        max_width: usize,
    },
    DrawText {
        text: String,
        color: [u8; 4],
        font: FontInput,
        scale: ScaleTuple,
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
    Rotate90,
    Rotate180,
    Rotate270,
}

impl ImageOperation {
    fn apply(self, mut image: DynamicImage) -> Result<DynamicImage, Errors> {
        match self {
            Self::Resize { h, w, filter } => Ok(image.resize(
                w,
                h,
                match filter.as_str() {
                    "Nearest" => Ok(FilterType::Nearest),
                    "Triangle" => Ok(FilterType::Triangle),
                    "CatmullRom" => Ok(FilterType::CatmullRom),
                    "Gaussian" => Ok(FilterType::Gaussian),
                    "Lanczos3" => Ok(FilterType::Lanczos3),
                    _ => Err(Errors::InvalidResizeFilter),
                }?,
            )),
            Self::Crop { x, y, w, h } => Ok(image.crop_imm(x, y, w, h)),
            Self::Overlay {
                layer_image_input,
                coords,
            } => {
                imageops::overlay(
                    &mut image,
                    &layer_image_input.get_image()?,
                    coords.0,
                    coords.1,
                );
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
                draw_text(
                    &mut image,
                    color,
                    &font.get_font()?,
                    final_text,
                    scale.to_scale(),
                    &mid,
                );
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
                draw_text(
                    &mut image,
                    color,
                    &font.get_font()?,
                    &text,
                    scale.to_scale(),
                    &mid,
                );
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
            Self::Rotate90 => Ok(image.rotate90()),
            Self::Rotate180 => Ok(image.rotate180()),
            Self::Rotate270 => Ok(image.rotate270()),
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
    Font::try_from_vec(fs::read(name)?.to_vec()).ok_or(Errors::InvalidFont)
}

pub fn fill_color(color: [u8; 3], size: (u32, u32)) -> RgbImage {
    let mut img = RgbImage::new(size.0, size.1);

    for x in 0..size.0 {
        for y in 0..size.1 {
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
    font.layout(text, scale, point(0.0, 0.0))
        .map(|g| g.position().x + g.unpositioned().h_metrics().advance_width)
        .last()
        .unwrap_or(0.0)
}

pub fn image_to_bytes(image: DynamicImage, format: ImageOutputFormat) -> Result<Vec<u8>, Errors> {
    let mut bytes: Vec<u8> = Vec::new();
    let mut w = Cursor::new(&mut bytes);
    image.write_to(&mut w, format)?;
    Ok(bytes)
}
