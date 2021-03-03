use cortex_m::interrupt::free;
use heapless::{ArrayLength, Vec};

pub fn get_current_ticks() -> u32 {
    free(|cs| {
        let mut data = crate::SYSTEM_DATA.borrow(cs).borrow_mut();
        data.get_mut().ticks_since_reset
    })
}

pub fn uart_buffer_push(c: u8) {
    free(|cs| {
        let mut buffer = crate::UART_BUFFER.borrow(cs).borrow_mut();
        buffer.push((c & 0xff) as u8);
    });
}

pub fn uart_buffer_pop<'t, N>() -> Vec<u8, N>
where
    N: ArrayLength<u8>,
{
    free(|cs| {
        let mut input_buffer = crate::UART_BUFFER.borrow(cs).borrow_mut();
        input_buffer.by_ref().take(9).collect()
    })
}
