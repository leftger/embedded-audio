/// Driver hook: apply one duty compare value per audio sample tick.
pub trait PwmDutySink {
    fn set_duty(&mut self, duty: u16);
}

impl<F: FnMut(u16)> PwmDutySink for F {
    fn set_duty(&mut self, duty: u16) {
        self(duty);
    }
}

/// Run one engine tick and push duty to the sink.
#[inline]
pub fn tick_into<S: PwmDutySink, const N: usize>(
    engine: &mut crate::AudioEngine<'_, N>,
    sink: &mut S,
) -> u16 {
    let duty = engine.tick();
    sink.set_duty(duty);
    duty
}

/// Fill `buf` via the engine and write each duty to the sink (DMA kick-off helper).
pub fn fill_buffer_into<S: PwmDutySink, const N: usize>(
    engine: &mut crate::AudioEngine<'_, N>,
    buf: &mut [u16],
    sink: &mut S,
) -> usize {
    let n = engine.fill_duty_buffer(buf);
    if let Some(&duty) = buf.last() {
        sink.set_duty(duty);
    }
    n
}

/// Double-buffer DMA helper to fill half-buffer callbacks (e.g. Embassy / STM32 DMA ISR).
pub fn fill_dma_half_buffers<const N: usize>(
    engine: &mut crate::AudioEngine<'_, N>,
    half_buf: &mut [u16],
) -> usize {
    engine.fill_duty_buffer(half_buf)
}

/// In-memory duty buffer for DMA (no hardware attached).
#[derive(Debug, PartialEq, Eq)]
pub struct DutyBuffer<'a> {
    pub buf: &'a mut [u16],
    pub cursor: usize,
}

impl<'a> DutyBuffer<'a> {
    pub const fn new(buf: &'a mut [u16]) -> Self {
        Self { buf, cursor: 0 }
    }
}

impl PwmDutySink for DutyBuffer<'_> {
    fn set_duty(&mut self, duty: u16) {
        if self.cursor < self.buf.len() {
            self.buf[self.cursor] = duty;
            self.cursor += 1;
        }
    }
}
