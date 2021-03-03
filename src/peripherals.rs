use shared_bus::{I2cProxy, NullMutex};
use ssd1306::{displaysize::DisplaySize128x32, prelude::*, Builder, I2CDIBuilder};
use stm32f4xx_hal::{
    gpio::{
        gpioa::{self, PA10, PA6, PA7, PA9},
        gpiob::{PB8, PB9},
        Alternate, AlternateOD, OpenDrain, Output, PushPull, AF4, AF7,
    },
    i2c::I2c,
    prelude::*,
    rcc::Clocks,
    serial::{
        self,
        config::{Parity, StopBits, WordLength},
        Event as SerialEvent, Serial,
    },
    stm32::{I2C1, TIM1, USART1},
    timer::Timer,
};

use crate::radiohead_ask;

type I2CInterfaceProxy<'t> =
    I2cProxy<'t, NullMutex<I2c<I2C1, (PB8<AlternateOD<AF4>>, PB9<AlternateOD<AF4>>)>>>;
type MLX90614<'t> = mlx9061x::Mlx9061x<I2CInterfaceProxy<'t>, mlx9061x::ic::Mlx90614>;
type DHT11 = dht11::Dht11<PA6<Output<OpenDrain>>>;
type UARTPins = (PA9<Alternate<AF7>>, PA10<Alternate<AF7>>);
pub type RadioHeadASK = radiohead_ask::RadioHeadASK<PA7<Output<PushPull>>, Timer<TIM1>>;


pub fn setup_display<I>(i2c: I) -> GraphicsMode<I2CInterface<I>, DisplaySize128x32>
where
    I: embedded_hal::blocking::i2c::Write,
{
    let interface = I2CDIBuilder::new().init(i2c);
    let mut disp: GraphicsMode<_, _> = Builder::new()
        .size(DisplaySize128x32)
        .connect(interface)
        .into();

    disp.init().unwrap();
    disp.flush().unwrap();
    disp
}

pub fn setup(
    gpioa: gpioa::Parts,
    i2c: I2CInterfaceProxy,
    clocks: Clocks,
    tim1: TIM1,
    usart1: USART1
) -> (MLX90614, DHT11, RadioHeadASK, Serial<USART1, UARTPins>) {
    let humidity_sensor: DHT11 = dht11::Dht11::new(gpioa.pa6.into_open_drain_output());
    let temperature_sensor =
        mlx9061x::Mlx9061x::new_mlx90614(i2c, mlx9061x::SlaveAddr::Alternative(0x5a), 5).unwrap();
    let uart_cfg = serial::config::Config {
        baudrate: 9600.bps(),
        parity: Parity::ParityNone,
        wordlength: WordLength::DataBits8,
        stopbits: StopBits::STOP1,
    };

    let timer = Timer::tim1(tim1, 200.khz(), clocks);

    let mut uart = serial::Serial::usart1(
        usart1,
        (
            gpioa.pa9.into_alternate_af7(),
            gpioa.pa10.into_alternate_af7(),
        ),
        uart_cfg,
        clocks,
    )
    .unwrap();
    uart.listen(SerialEvent::Rxne);

    let radio = radiohead_ask::RadioHeadASK::new(gpioa.pa7.into_push_pull_output(), timer);

    (temperature_sensor, humidity_sensor, radio, uart)
}
