use crate::syscalls;
use core::fmt::Debug;
use cortex_m::interrupt::free;
use embedded_hal::serial::{Read, Write};
use heapless::{consts::*, Vec};
use nb::block;

#[derive(Debug)]
pub enum Error {
    IncompletePacket,
    WrongStartByte,
    WrongChecksum,
    Timeout,
}

fn calc_checksum(buf: &[u8]) -> u8 {
    1u8.wrapping_add(0xff - buf.iter().fold(0u8, |sum, b| sum.wrapping_add(*b)))
}

pub fn parse_data(buf: &[u8]) -> Result<u16, Error> {
    if buf.len() < 9 {
        Err(Error::IncompletePacket)
    } else if buf[0] != 0xff {
        Err(Error::WrongStartByte)
    } else if buf[8] != calc_checksum(&buf[1..8]) {
        Err(Error::WrongChecksum)
    } else {
        Ok(((buf[2] as u16) << 8) | buf[3] as u16)
    }
}

pub fn request_reading<U, E>(uart: &mut U) -> Result<u16, Error>
where
    U: Read<u8, Error = E> + Write<u8, Error = E>,
    E: Debug,
{
    // we can clean up the UART buffer
    free(|cs| {
        let mut input_buffer = crate::UART_BUFFER.borrow(cs).borrow_mut();
        input_buffer.clear();
    });

    for c in &[0xFF, 0x01, 0x86, 0, 0, 0, 0, 0, 0x79] {
        block!(uart.write(*c)).unwrap();
    }
    block!(uart.flush()).unwrap();

    let mut buf = Vec::<u8, U9>::new();
    let mut start_time = syscalls::get_current_ticks();

    loop {
        buf.extend_from_slice(&syscalls::uart_buffer_pop::<U9>())
            .unwrap();

        let current_ticks = syscalls::get_current_ticks();
        if start_time > current_ticks {
            // the tick counter wrapped around
            start_time = current_ticks;
        }

        if buf.len() == 9 {
            break Ok(parse_data(&buf)?);
        } else if current_ticks - start_time > 5 {
            break Err(Error::Timeout);
        }
    }
}
