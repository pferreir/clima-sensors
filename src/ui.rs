use core::cell::RefCell;
use cortex_m::interrupt::{free, Mutex};
use embedded_graphics::{egtext, image::Image, pixelcolor::BinaryColor, prelude::*, text_style};
use heapless::{consts::*, HistoryBuffer, String};
use profont::{ProFont12Point, ProFont9Point};
use ssd1306::{mode::GraphicsMode, prelude::*};
use tinybmp::Bmp;
use ufmt::uwrite;

use crate::SystemData;

const NUM_LOG_LINES: usize = 4;

lazy_static! {
    static ref RF_BMP: Bmp<'static> = Bmp::from_slice(include_bytes!("./radio.bmp")).unwrap();
    static ref LOG_BUFFER: Mutex<RefCell<HistoryBuffer<String<U32>, U4>>> =
        Mutex::new(RefCell::new(HistoryBuffer::new()));
}

pub struct Ui<'t, I>
where
    I: WriteOnlyDataCommand,
{
    rf_icon: Image<'t, Bmp<'static>, BinaryColor>,
    display: GraphicsMode<I, DisplaySize128x32>,
}

impl<'t, I> Ui<'t, I>
where
    I: WriteOnlyDataCommand,
{
    pub fn new(display: GraphicsMode<I, DisplaySize128x32>) -> Self {
        Self {
            display,
            rf_icon: Image::new(&RF_BMP, Point::new(105, 20)),
        }
    }

    pub fn clear(&mut self) {
        self.display.clear();
    }

    pub fn flush(&mut self) {
        self.display.flush().unwrap();
    }

    pub fn draw(&mut self, system_data: &SystemData) {
        let mut text: String<U16> = String::new();

        if system_data.errors.temperature {
            uwrite!(&mut text, "ERR").unwrap();
        } else {
            let t = system_data.sensors.avgs.temperature;
            let t_int = t / 100;
            let t_dec = t % 100;
            uwrite!(&mut text, "{}.{}C", t_int, t_dec).unwrap();
        }

        egtext!(
            text = &text,
            top_left = (0, 0),
            style = text_style!(font = ProFont12Point, text_color = BinaryColor::On)
        )
        .draw(&mut self.display)
        .unwrap();

        text.clear();

        if system_data.errors.humidity {
            uwrite!(&mut text, "ERR").unwrap();
        } else {
            let h = system_data.sensors.avgs.humidity;
            uwrite!(&mut text, "{}%", h).unwrap();
        }

        egtext!(
            text = &text,
            top_left = (95, 0),
            style = text_style!(font = ProFont12Point, text_color = BinaryColor::On)
        )
        .draw(&mut self.display)
        .unwrap();

        text.clear();

        if system_data.errors.co2 {
            uwrite!(&mut text, "ERR").unwrap();
        } else {
            let c = system_data.sensors.avgs.co2;
            uwrite!(&mut text, "{}ppm", c).unwrap();
        }

        // match system_data.sensors.co2.recent() {
        //     Some(c) => {
        //         uwrite!(&mut text, "{}ppm", c).unwrap();
        //     },
        //     None => {
        //         uwrite!(text, "ERR").unwrap();
        //     }
        // }

        egtext!(
            text = &text,
            top_left = (00, 18),
            style = text_style!(font = ProFont12Point, text_color = BinaryColor::On)
        )
        .draw(&mut self.display)
        .unwrap();

        if system_data.ticks_since_last_tx < 10 {
            self.rf_icon.draw(&mut self.display).unwrap();
        }
    }

    pub fn log_to_screen(&mut self, text: &str) {
        self.display.clear();

        free(|cs| {
            let mut buffer = LOG_BUFFER.borrow(cs).borrow_mut();

            buffer.write(String::from(text));

            for i in 0..NUM_LOG_LINES {
                egtext!(
                    text = &buffer.as_slice()[i],
                    top_left = (0, (i * 8) as i32),
                    style = text_style!(font = ProFont9Point, text_color = BinaryColor::On)
                )
                .draw(&mut self.display)
                .unwrap();
            }
        });

        self.display.flush().unwrap();
    }
}
