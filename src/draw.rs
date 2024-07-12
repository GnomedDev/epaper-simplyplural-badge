use core::convert::Infallible;

use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::Point,
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    text::Text,
    Drawable as _,
};
use epd_waveshare::color::Color;

use crate::EpdBuffer;

fn into_ok<T>(res: Result<T, Infallible>) -> T {
    match res {
        Ok(val) => val,
        Err(err) => match err {},
    }
}

pub fn clear_display(display: &mut EpdBuffer) {
    into_ok(display.clear(Color::White))
}

pub fn text_to_display(display: &mut EpdBuffer, text: &str) {
    let style = MonoTextStyle::new(&FONT_10X20, Color::Black);

    into_ok(Text::new(text, Point::new(20, 30), style).draw(display));
}
