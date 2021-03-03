#![no_main]
#![no_std]

#[macro_use]
extern crate lazy_static;

#[cfg(debug_assertions)]
extern crate panic_itm;

#[cfg(not(debug_assertions))]
extern crate panic_halt;

use cmim::{Context, Move};
use core::{
    cell::{Cell, RefCell},
    cmp, mem, slice,
};
use cortex_m::{
    interrupt::{free, Mutex},
    iprintln,
};
use cortex_m_rt::entry;
use heapless::{consts::*, HistoryBuffer, Vec};
use shared_bus::BusManagerSimple;
use stm32f4xx_hal::{
    delay::Delay,
    i2c::I2c,
    interrupt,
    prelude::*,
    stm32::{self, NVIC},
    timer::{Event, Timer},
};

const TICKS_UNTIL_RADIO_TX: u32 = 50; // 10 s
const TICKS_UNTIL_SENSOR_READ: u32 = 10; // 2s

mod mhz19b;
mod peripherals;
mod radiohead_ask;
mod ringbuffer;
mod syscalls;
mod ui;

pub struct Averages {
    pub temperature: i16,
    pub humidity: u16,
    pub co2: u16,
}

impl Averages {
    pub fn new() -> Self {
        Self {
            temperature: 0,
            humidity: 0,
            co2: 0,
        }
    }
}

pub struct SensorData {
    pub temperature: HistoryBuffer<i16, U8>,
    pub humidity: HistoryBuffer<u16, U8>,
    pub co2: HistoryBuffer<u16, U8>,
    pub num_points: u8,
    pub avgs: Averages,
}

impl SensorData {
    pub fn new() -> Self {
        Self {
            temperature: HistoryBuffer::new_with(0),
            humidity: HistoryBuffer::new_with(0),
            co2: HistoryBuffer::new_with(0),
            avgs: Averages::new(),
            num_points: 0,
        }
    }

    pub fn recalc_averages(&mut self) {
        self.avgs.temperature =
            self.temperature.as_slice().iter().sum::<i16>() / self.num_points as i16;
        self.avgs.humidity = self.humidity.as_slice().iter().sum::<u16>() / self.num_points as u16;
        self.avgs.co2 = self.co2.as_slice().iter().sum::<u16>() / self.num_points as u16;
    }
}

struct ErrorData {
    temperature: bool,
    humidity: bool,
    co2: bool,
}

impl ErrorData {
    pub fn new() -> Self {
        Self {
            temperature: false,
            humidity: false,
            co2: false,
        }
    }
}

pub struct SystemData {
    pub ticks_since_reset: u32,
    ticks_since_last_tx: u32,
    ticks_since_last_read: u32,
    send_tx_now: bool,
    read_sensors_now: bool,
    sensors: SensorData,
    errors: ErrorData,
}

impl SystemData {
    fn new() -> Self {
        Self {
            ticks_since_reset: 0,
            ticks_since_last_tx: 0,
            ticks_since_last_read: 0,
            send_tx_now: false,
            read_sensors_now: false,
            sensors: SensorData::new(),
            errors: ErrorData::new(),
        }
    }
}

static TIMER_TIM2: Move<Timer<stm32::TIM2>, stm32::Interrupt> =
    Move::new_uninitialized(Context::Interrupt(stm32::Interrupt::TIM2));

lazy_static! {
    static ref SYSTEM_DATA: Mutex<RefCell<Cell<SystemData>>> =
        Mutex::new(RefCell::new(Cell::new(SystemData::new())));
}

lazy_static! {
    static ref UART_BUFFER: Mutex<RefCell<ringbuffer::RingBuffer<u8>>> =
        Mutex::new(RefCell::new(ringbuffer::RingBuffer::new()));
}

fn send_radio_packet<T>(radio: &mut peripherals::RadioHeadASK, id: u8, value: T) {
    radio
        .send_packet(0xff, 0xff, id, 0, &num_to_bytes(value))
        .unwrap();
}

fn num_to_bytes<T>(value: T) -> Vec<u8, U4> {
    let v: *const T = &value;
    let b_val: *const u8 = v as *const _;
    // This is badass unsafe. Just doing it because I didn't want yet another dependency
    let slice: &[u8] = unsafe { slice::from_raw_parts(b_val, mem::size_of::<T>()) };

    Vec::from_slice(slice).unwrap()
}

fn write_value<T, E>(
    history: &mut HistoryBuffer<T, U8>,
    error_flag: &mut bool,
    data: Result<T, E>,
) {
    match data {
        Ok(d) => {
            history.write(d);
            *error_flag = false;
        }
        Err(_) => {
            *error_flag = true;
        }
    }
}

#[entry]
fn main() -> ! {
    if let (Some(p), Some(mut cp)) = (stm32::Peripherals::take(), cortex_m::Peripherals::take()) {
        // Constrain clocking registers
        let rcc = p.RCC.constrain();
        let clocks = rcc.cfgr.sysclk(84.mhz()).freeze();
        let gpiob = p.GPIOB.split();
        let mut delay = Delay::new(cp.SYST, clocks);

        // I2C
        let scl = gpiob.pb8.into_alternate_af4().set_open_drain();
        let sda = gpiob.pb9.into_alternate_af4().set_open_drain();

        let i2c_bus = BusManagerSimple::new(I2c::i2c1(p.I2C1, (scl, sda), 100.khz(), clocks));

        // Initialize display
        let disp = peripherals::setup_display(i2c_bus.acquire_i2c());
        let mut ui = ui::Ui::new(disp);
        ui.log_to_screen("Display init'd");

        // TIM2 is used as a system timer
        let mut tim2 = Timer::tim2(p.TIM2, 5.hz(), clocks);
        tim2.listen(Event::TimeOut);
        // move the timer to the exception handler
        TIMER_TIM2.try_move(tim2).ok();

        let gpioa = p.GPIOA.split();

        NVIC::unpend(stm32::Interrupt::TIM2);
        unsafe {
            NVIC::unmask(stm32::Interrupt::TIM2);
            NVIC::unmask(stm32::Interrupt::USART1);
        };

        ui.log_to_screen("Interrupts set");

        let (mut temperature_sensor, mut humidity_sensor, mut radio, mut uart) = peripherals::setup(
            gpioa,
            i2c_bus.acquire_i2c(),
            clocks,
            p.TIM1,
            p.USART1
        );

        ui.log_to_screen("Peripherals init'd");

        #[cfg(debug_assertions)]
        let itm = &mut cp.ITM.stim[0];

        // Main loop
        loop {
            let (read_sensors_now, send_tx_now) = free(|cs| {
                let mut data = SYSTEM_DATA.borrow(cs).borrow_mut();
                let data = data.get_mut();
                (data.read_sensors_now, data.send_tx_now)
            });

            if read_sensors_now {
                let temperature = temperature_sensor.ambient_temperature();
                let humidity = humidity_sensor.perform_measurement(&mut delay);
                let co2 = mhz19b::request_reading(&mut uart);

                free(|cs| {
                    let mut data = SYSTEM_DATA.borrow(cs).borrow_mut();
                    let mut data = data.get_mut();

                    data.read_sensors_now = false;

                    write_value(
                        &mut data.sensors.temperature,
                        &mut data.errors.temperature,
                        temperature.map(|v| (v * 100.0) as i16),
                    );
                    write_value(
                        &mut data.sensors.humidity,
                        &mut data.errors.humidity,
                        humidity.map(|v| v.humidity / 10),
                    );
                    write_value(&mut data.sensors.co2, &mut data.errors.co2, co2);
                    data.sensors.num_points = cmp::min(data.sensors.num_points + 1, 8);
                    data.sensors.recalc_averages();

                    #[cfg(debug_assertions)]
                    iprintln!(
                        itm,
                        "LAST T:{}C H:{}% CO2:{}ppm",
                        data.sensors.temperature.recent(),
                        data.sensors.humidity.recent(),
                        data.sensors.co2.recent()
                    );
                    #[cfg(debug_assertions)]
                    iprintln!(
                        itm,
                        "AVRG T:{}C H:{}% CO2:{}ppm",
                        data.sensors.avgs.temperature,
                        data.sensors.avgs.humidity,
                        data.sensors.avgs.co2
                    );
                });
            }

            if send_tx_now {
                free(|cs| {
                    let mut data = SYSTEM_DATA.borrow(cs).borrow_mut();
                    let mut data = data.get_mut();

                    send_radio_packet(&mut radio, 0xed, data.sensors.avgs.temperature);
                    delay.delay_ms(100u16);
                    send_radio_packet(&mut radio, 0xee, data.sensors.avgs.humidity);
                    delay.delay_ms(100u16);
                    send_radio_packet(&mut radio, 0xef, data.sensors.avgs.co2);

                    // reset send flag
                    data.send_tx_now = false;
                });
            }

            ui.clear();

            free(|cs| {
                let mut data = SYSTEM_DATA.borrow(cs).borrow_mut();
                ui.draw(&data.get_mut());
            });

            ui.flush();
            delay.delay_ms(10_u16);
        }
    }
    loop {
        panic!("Can't get peripherals!");
    }
}

#[interrupt]
fn TIM2() {
    TIMER_TIM2
        .try_lock(|tim2| {
            free(|cs| {
                let mut data = SYSTEM_DATA.borrow(cs).borrow_mut();
                let data = data.get_mut();

                data.ticks_since_reset += 1;

                if data.ticks_since_last_tx > TICKS_UNTIL_RADIO_TX {
                    data.ticks_since_last_tx = 0;
                    data.send_tx_now = true;
                } else {
                    data.ticks_since_last_tx += 1;
                }

                if data.ticks_since_last_read > TICKS_UNTIL_SENSOR_READ {
                    data.ticks_since_last_read = 0;
                    data.read_sensors_now = true;
                } else {
                    data.ticks_since_last_read += 1;
                }
            });

            tim2.clear_interrupt(Event::TimeOut);
        })
        .ok();
}

#[interrupt]
fn USART1() {
    let c = unsafe {
        let p = stm32::Peripherals::steal();
        p.USART1.dr.read().bits()
    };
    syscalls::uart_buffer_push((c & 0xff) as u8);
}
