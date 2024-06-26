use std::{default::Default, fs, io::Cursor};

use conv::ValueInto;
use image::imageops::FilterType;
pub use image::{
    imageops, io::Reader, DynamicImage, GenericImage, GenericImageView, ImageOutputFormat, Pixel,
    Rgb, RgbImage, Rgba,
};
pub use imageproc::{definitions::Clamp, drawing::draw_text_mut};
pub use rusttype::{point, Font, Scale};
#[cfg(feature = "serde")]
use serde::Deserialize;

pub mod build_info;
pub mod errors;

pub use crate::errors::Errors;

#[cfg_attr(
    feature = "serde",
    derive(Deserialize),
    serde(rename_all = "snake_case")
)]
#[derive(Default)]
pub enum ResizeMode {
    #[default]
    Fit,
    Exact,
    Fill,
}

#[cfg_attr(
    feature = "serde",
    derive(Deserialize),
    serde(rename_all = "snake_case")
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
    serde(rename_all = "snake_case")
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
    #[cfg_attr(all(feature = "serde", not(feature = "serde_file")), serde(skip))]
    Filename(String),
    #[cfg_attr(feature = "serde", serde(skip_deserializing))]
    Bytes(Vec<u8>),
    New {
        h: u32,
        w: u32,
        type_: String,
    },
    #[cfg(feature = "base64")]
    Base64(String),
    #[cfg(feature = "reqwest")]
    Url(String),
}

macro_rules! new_image{
    ( $type_: ident, $h:ident, $w:ident, $( $x:ident ),* ) => {
        {
            match $type_.as_str() {
                $(
                    stringify!($x) => Ok(image::$x::new($w, $h).into()),
                )*
                _ => Err(Errors::InvalidImageType)
            }
        }
    };
}

impl ImageInputType {
    pub fn get_image(self) -> Result<DynamicImage, Errors> {
        match self {
            Self::DynamicImage(image) => Ok(image),
            Self::Color { r, g, b, size } => {
                Ok(DynamicImage::ImageRgb8(fill_color([r, g, b], size)))
            }
            Self::Filename(name) => load_image_from_file(&name),
            Self::Bytes(bytes) => Ok(image::load_from_memory(&bytes)?),
            Self::New { h, w, type_ } => new_image!(
                type_,
                h,
                w,
                RgbImage,
                RgbaImage,
                GrayImage,
                GrayAlphaImage,
                Rgb32FImage,
                Rgba32FImage
            ),
            #[cfg(feature = "base64")]
            Self::Base64(encoded) => Ok(image::load_from_memory(&base64::decode(encoded)?)?),
            #[cfg(feature = "reqwest")]
            Self::Url(url) => Ok(image::load_from_memory(
                &reqwest::blocking::get(url)?.bytes()?,
            )?),
        }
    }
}

#[cfg_attr(
    feature = "serde",
    derive(Deserialize),
    serde(rename_all = "snake_case")
)]
pub enum FontInput {
    #[cfg_attr(feature = "serde", serde(skip_deserializing))]
    Font(Font<'static>),
    #[cfg_attr(all(feature = "serde", not(feature = "serde_file")), serde(skip))]
    Filename(String),
    #[cfg_attr(feature = "serde", serde(skip_deserializing))]
    Bytes(Vec<u8>),
    #[cfg(feature = "base64")]
    Base64(String),
    #[cfg(feature = "reqwest")]
    Url(String),
}

impl FontInput {
    pub fn get_font(self) -> Result<Font<'static>, Errors> {
        match self {
            Self::Font(font) => Ok(font),
            Self::Filename(name) => load_font_from_file(&name),
            Self::Bytes(bytes) => Font::try_from_vec(bytes).ok_or(Errors::InvalidFont),
            #[cfg(feature = "base64")]
            Self::Base64(encoded) => {
                Font::try_from_vec(base64::decode(encoded)?).ok_or(Errors::InvalidFont)
            }
            #[cfg(feature = "reqwest")]
            Self::Url(url) => Font::try_from_vec(reqwest::blocking::get(url)?.bytes()?.to_vec())
                .ok_or(Errors::InvalidFont),
        }
    }
}

#[cfg_attr(
    feature = "serde",
    derive(Deserialize),
    serde(rename_all = "snake_case")
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
    serde(rename_all = "snake_case")
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
    serde(rename_all = "snake_case")
)]
pub enum ImageOperation {
    Thumbnail {
        w: u32,
        h: u32,
        #[cfg_attr(feature = "serde", serde(default))]
        exact: bool,
    },
    Resize {
        h: u32,
        w: u32,
        filter: String,
        #[cfg_attr(feature = "serde", serde(default))]
        mode: ResizeMode,
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
    Tile {
        tile_image: ImageInput,
    },
    DrawText {
        text: String,
        color: [u8; 4],
        font: FontInput,
        scale: ScaleTuple,
        mid: (i32, i32),
        max_width: Option<usize>,
    },
    ColorBlend {
        r: u8,
        g: u8,
        b: u8,
    },
    Blur {
        sigma: f32,
    },
    Unsharpen {
        sigma: f32,
        threshold: i32,
    },
    Brighten(i32),
    AdjustContrast(f32),
    HueRotate(i32),
    Invert,
    Grayscale,
    FlipHorizontal,
    FlipVertical,
    Rotate90,
    Rotate180,
    Rotate270,
}

impl ImageOperation {
    fn apply(self, mut image: DynamicImage) -> Result<DynamicImage, Errors> {
        match self {
            Self::Thumbnail { h, w, exact } => Ok(if exact {
                image.thumbnail_exact(w, h)
            } else {
                image.thumbnail(w, h)
            }),
            Self::Resize { h, w, filter, mode } => {
                let func = match mode {
                    ResizeMode::Fit => DynamicImage::resize,
                    ResizeMode::Exact => DynamicImage::resize_exact,
                    ResizeMode::Fill => DynamicImage::resize_to_fill,
                };
                Ok(func(&image, w, h, filter_from_str(filter)?))
            }
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
            Self::Tile { tile_image } => {
                image::imageops::tile(&mut image, &tile_image.get_image()?);
                Ok(image)
            }
            Self::DrawText {
                mut text,
                color,
                font,
                scale,
                mid,
                max_width,
            } => {
                if let Some(width) = max_width {
                    text = textwrap::fill(&text, width);
                }
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
            Self::Blur { sigma } => Ok(image.blur(sigma)),
            Self::Unsharpen { sigma, threshold } => {
                Ok(image::imageops::unsharpen(&image, sigma, threshold).into())
            }
            Self::Brighten(value) => Ok(image.brighten(value)),
            Self::AdjustContrast(value) => Ok(image.adjust_contrast(value)),
            Self::HueRotate(value) => Ok(image.huerotate(value)),
            Self::Invert => {
                image.invert();
                Ok(image)
            }
            Self::Grayscale => Ok(image::imageops::grayscale(&image).into()),
            Self::FlipHorizontal => Ok(image.fliph()),
            Self::FlipVertical => Ok(image.flipv()),
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

fn filter_from_str(filter: String) -> Result<FilterType, Errors> {
    match filter.as_str() {
        "Nearest" => Ok(FilterType::Nearest),
        "Triangle" => Ok(FilterType::Triangle),
        "CatmullRom" => Ok(FilterType::CatmullRom),
        "Gaussian" => Ok(FilterType::Gaussian),
        "Lanczos3" => Ok(FilterType::Lanczos3),
        _ => Err(Errors::InvalidResizeFilter),
    }
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
