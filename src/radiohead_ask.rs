use embedded_hal::{
    digital::v2::OutputPin,
    timer::{CountDown, Periodic},
};
use nb::block;
use stm32f4xx_hal::{prelude::*, time::Hertz};

const SYMBOLS: [u8; 16] = [
    0xd, 0xe, 0x13, 0x15, 0x16, 0x19, 0x1a, 0x1c, 0x23, 0x25, 0x26, 0x29, 0x2a, 0x2c, 0x32, 0x34,
];

#[derive(Debug)]
pub enum Error {
    Port,
}

pub struct RadioHeadASK<P, T>
where
    P: OutputPin,
    T: CountDown + Periodic,
{
    pin: P,
    timer: T,
    fcs: u16,
    buffer: [u8; 128],
    buffer_ptr: usize,
}

impl<P, T> RadioHeadASK<P, T>
where
    P: OutputPin,
    T: CountDown<Time = Hertz> + Periodic,
{
    pub fn new(mut pin: P, timer: T) -> Self {
        match pin.set_low() {
            Ok(_) => {}
            Err(_) => panic!("Couldn't set pin to low!"),
        }

        Self {
            pin,
            timer,
            fcs: 0xffff,
            buffer: [0_u8; 128],
            buffer_ptr: 0,
        }
    }

    fn update_fcs(&mut self, data: u8) {
        let mut new_data = data ^ (self.fcs as u8);
        new_data ^= new_data << 4;
        self.fcs = (((new_data as u16) << 8) | (self.fcs >> 8))
            ^ ((new_data >> 4) as u16)
            ^ ((new_data as u16) << 3);
    }

    pub fn send_packet(
        &mut self,
        from: u8,
        to: u8,
        id: u8,
        header_flags: u8,
        content: &[u8],
    ) -> Result<(), Error> {
        self.buffer_ptr = 0;

        self.fcs = 0xffff;

        for _i in 0..6 {
            // bits will be inverted upon sending
            self.enqueue_sextet(0x2a);
        }
        self.enqueue_sextet(0x38);
        self.enqueue_sextet(0x2c);

        let msg_len = content.len() + 7;
        self.enqueue_bytes(&[msg_len as u8, from, to, id, header_flags])?;
        self.enqueue_bytes(content)?;

        let crc = !self.fcs;
        self.enqueue_bytes(&[(crc & 0xff) as u8, (crc >> 8) as u8])?;

        self.flush()?;

        Ok(())
    }

    fn set_bit(&mut self, bit: bool) -> Result<(), Error> {
        if bit {
            self.pin.set_high().map_err(|_| Error::Port)?;
        } else {
            self.pin.set_low().map_err(|_| Error::Port)?;
        }
        Ok(())
    }

    fn enqueue_sextet(&mut self, sextet: u8) {
        self.buffer[self.buffer_ptr] = sextet;
        self.buffer_ptr += 1;
    }

    fn flush(&mut self) -> Result<(), Error> {
        self.timer.start(2000.hz());

        for n in 0..self.buffer_ptr {
            for i in 0..6 {
                self.set_bit((self.buffer[n] & (0x01 << i)) > 0)?;
                block!(self.timer.wait()).unwrap();
            }
        }
        self.set_bit(false).unwrap();
        Ok(())
    }

    fn enqueue_bytes(&mut self, bytes: &[u8]) -> Result<(), Error> {
        bytes.iter().for_each(|b| {
            self.enqueue_sextet(SYMBOLS[(b >> 4) as usize]);
            self.enqueue_sextet(SYMBOLS[(b & 0xf) as usize]);
            self.update_fcs(*b);
        });
        Ok(())
    }
}
