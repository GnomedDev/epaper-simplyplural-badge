use core::convert::Infallible;

use embedded_font::FontTextStyleBuilder;
use embedded_graphics::{draw_target::DrawTarget, geometry::Point, text::Text, Drawable as _};
use epd_waveshare::color::Color;

use crate::EpdBuffer;

pub enum FontSize {
    Small = 24,
    Large = 50,
}

fn into_ok<T>(res: Result<T, Infallible>) -> T {
    match res {
        Ok(val) => val,
        Err(err) => match err {},
    }
}

pub fn clear_display(display: &mut EpdBuffer) {
    into_ok(display.clear(Color::White));
}

pub fn text_to_display(
    display: &mut EpdBuffer,
    font: rusttype::Font<'static>,
    font_size: FontSize,
    text: &str,
) {
    let style = FontTextStyleBuilder::new(font)
        .text_color(Color::Black)
        .font_size(font_size as u32)
        .build();

    into_ok(Text::new(text, Point::new(20, 40), style).draw(display));
}
