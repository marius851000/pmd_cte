use byteorder::{ReadBytesExt, WriteBytesExt, LE};
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba};
use std::io::Read;
use std::io::{self, Write};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CteDecodeError {
    #[error("An issue occured when reading the file")]
    IOError(#[from] io::Error),
    #[error("the header of the cte file doesn't correspond to the expected one (\\x0cte): {0:?}")]
    InvalideHeader([u8; 4]),
    #[error("the cte image format with the id {0} isn't supported")]
    UnsuportedFormat(u32),
    #[error("the cte image content is mixed with the header. That shouldn't happen. (the content start at {0})")]
    ImageStartTooSoon(u32),
    #[error(
        "the number of byte by pixel in the file is invalid (the size is {0}, for the format {1:?}"
    )]
    PixelLenghtInvalid(u32, CteFormat),
    #[error("the width {0} of the image isn't a multiple of 8")]
    WidthNotMultiple8(u32),
    #[error("the height {0} of the image isn't a multiple of 8")]
    HeightNotMultiple8(u32),
    #[error("internal error : {0}")]
    InternalError(&'static str),
}

#[derive(Error, Debug)]
pub enum CteEncodeError {
    #[error("An issue occured while writing the file")]
    IOError(#[from] io::Error),
    #[error("the width {0} of the image isn't a multiple of 8")]
    WidthNotMultiple8(u32),
    #[error("the height {0} of the image isn't a multiple of 8")]
    HeightNotMultiple8(u32),
}

fn read_in_image_order<B, F>(buffer: &[B; 64], mut func: F)
where
    B: Clone,
    F: FnMut(u32, u32, B),
{
    let mut iterator = buffer.iter();
    for pair1 in &[(0, 4), (4, 4), (0, 0), (4, 0)] {
        for pair2 in &[(0, 2), (2, 2), (0, 0), (2, 0)] {
            for pair3 in &[(0, 1), (1, 1), (0, 0), (1, 0)] {
                let x = pair1.0 + pair2.0 + pair3.0;
                let y = pair1.1 + pair2.1 + pair3.1;
                func(x, y, iterator.next().unwrap().clone());
            }
        }
    }
}

#[derive(Debug)]
pub enum CteFormat {
    A8,
}

impl CteFormat {
    pub fn from_id(id: u32) -> Option<Self> {
        Some(match id {
            8 => Self::A8,
            _ => return None,
        })
    }

    pub fn get_id(&self) -> u32 {
        match self {
            Self::A8 => 8,
        }
    }

    pub fn check_pixel_lenght_bit(&self, lenght: u32) -> bool {
        lenght == self.get_pixel_length_bit()
    }

    pub fn get_pixel_length_bit(&self) -> u32 {
        match self {
            Self::A8 => 8,
        }
    }
}

const CTE_HEADER_SIZE: u8 = 28;
const CTE_HEADER: [u8; 4] = [0x0, 0x63, 0x74, 0x65];

pub struct CteImage {
    pub original_format: CteFormat,
    pub image: DynamicImage,
}

impl CteImage {
    pub fn decode_cte<R: Read>(input: &mut R) -> Result<CteImage, CteDecodeError> {
        let mut header_buffer = [0; 4];
        input.read_exact(&mut header_buffer)?;
        if header_buffer != CTE_HEADER {
            return Err(CteDecodeError::InvalideHeader(header_buffer));
        };
        let format_id = input.read_u32::<LE>()?;
        let image_format = if let Some(f) = CteFormat::from_id(format_id) {
            f
        } else {
            return Err(CteDecodeError::UnsuportedFormat(format_id));
        };

        let width = input.read_u32::<LE>()?;
        let height = input.read_u32::<LE>()?;
        let pixel_lenght = input.read_u32::<LE>()?;
        let _unk = input.read_u32::<LE>()?;
        let pixel_start_offset = input.read_u32::<LE>()?;

        if !image_format.check_pixel_lenght_bit(pixel_lenght) {
            return Err(CteDecodeError::PixelLenghtInvalid(
                pixel_lenght,
                image_format,
            ));
        };

        let distance_before_start = pixel_start_offset
            .checked_sub(CTE_HEADER_SIZE as u32)
            .map_or_else(
                || Err(CteDecodeError::ImageStartTooSoon(pixel_start_offset)),
                Ok,
            )?;
        input.read_exact(&mut vec![0; distance_before_start as usize])?;

        if width % 8 != 0 {
            return Err(CteDecodeError::WidthNotMultiple8(width));
        };
        if height % 8 != 0 {
            return Err(CteDecodeError::HeightNotMultiple8(height));
        };
        let width_section = width / 8;
        let height_section = height / 8;
        let image = match image_format {
            CteFormat::A8 => {
                let mut image: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width, height);
                for y in (0..height_section).rev() {
                    for x in 0..width_section {
                        let mut section = [0; 64];
                        input.read_exact(&mut section)?;
                        let start_x = x * 8;
                        let start_y = y * 8;
                        let image_ref = &mut image;
                        read_in_image_order(&section, move |x, y, v| {
                            let alpha = (v % 16) * 16;
                            let white = v / 16;
                            image_ref.put_pixel(
                                start_x + x,
                                start_y + y,
                                Rgba([white, white, white, alpha]),
                            )
                        });
                    }
                }
                DynamicImage::ImageRgba8(image)
            }
        };
        Ok(CteImage {
            image,
            original_format: image_format,
        })
    }

    pub fn encode_cte<W: Write>(&self, out: &mut W) -> Result<(), CteEncodeError> {
        out.write_all(&CTE_HEADER)?;
        out.write_u32::<LE>(self.original_format.get_id())?;
        out.write_u32::<LE>(self.image.width())?;
        out.write_u32::<LE>(self.image.height())?;
        out.write_u32::<LE>(self.original_format.get_pixel_length_bit())?;
        out.write_u32::<LE>(0)?;
        out.write_u32::<LE>(128)?;
        let padding = [0; 128 - (CTE_HEADER_SIZE as usize)];
        out.write_all(&padding)?;
        if self.image.width() % 8 != 0 {
            return Err(CteEncodeError::WidthNotMultiple8(self.image.width()));
        };
        if self.image.height() % 8 != 0 {
            return Err(CteEncodeError::HeightNotMultiple8(self.image.height()));
        };
        let height_section = self.image.height() / 8;
        let width_section = self.image.width() / 8;
        for y_base in (0..height_section).rev() {
            for x_base in 0..width_section {
                let x_base = x_base * 8;
                let y_base = y_base * 8;
                for pair1 in &[(0, 4), (4, 4), (0, 0), (4, 0)] {
                    for pair2 in &[(0, 2), (2, 2), (0, 0), (2, 0)] {
                        for pair3 in &[(0, 1), (1, 1), (0, 0), (1, 0)] {
                            let x_coord = x_base + pair1.0 + pair2.0 + pair3.0;
                            let y_coord = y_base + pair1.1 + pair2.1 + pair3.1;
                            match self.original_format {
                                CteFormat::A8 => {
                                    let pixel = self.image.get_pixel(x_coord, y_coord).0;
                                    let white =
                                        ((pixel[0] as u16 + pixel[1] as u16 + pixel[2] as u16) / 3)
                                            as u8;
                                    let alpha = pixel[3];
                                    let to_write = white.overflowing_shl(4).0 + (alpha / 16);
                                    out.write_u8(to_write)?; //TODO: find a clean way to handle those colors
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
