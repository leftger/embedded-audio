/// Driver hook: apply one duty compare value per audio sample tick.
pub trait PwmDutySink {
    fn set_duty(&mut self, duty: u16);
}

/// Run one engine tick and push duty to the sink.
#[inline]
pub fn tick_into<S: PwmDutySink>(engine: &mut crate::AudioEngine<'_>, sink: &mut S) -> u16 {
    let duty = engine.tick();
    sink.set_duty(duty);
    duty
}

/// Fill `buf` via the engine and write each duty to the sink (DMA kick-off helper).
pub fn fill_buffer_into<S: PwmDutySink>(
    engine: &mut crate::AudioEngine<'_>,
    buf: &mut [u16],
    sink: &mut S,
) -> usize {
    let n = engine.fill_duty_buffer(buf);
    if let Some(&duty) = buf.last() {
        sink.set_duty(duty);
    }
    n
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
