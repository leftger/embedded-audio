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

/// Double-buffer ping-pong manager for DMA audio streaming.
///
/// Designed to work seamlessly with async DMA drivers (e.g. Embassy `dac.write()`,
/// `sai.write()`, `timer.write_dma()`, etc.) or IRQ-driven DMA half-transfer callbacks.
#[derive(Debug, Clone)]
pub struct DmaDoubleBuffer<T, const N: usize> {
    buffer: [[T; N]; 2],
    active_half: usize,
}

impl<T: Copy + Default, const N: usize> DmaDoubleBuffer<T, N> {
    pub const HALF_SIZE: usize = N;
    pub const TOTAL_SIZE: usize = N * 2;

    pub fn new() -> Self {
        Self {
            buffer: [[T::default(); N]; 2],
            active_half: 0,
        }
    }

    pub const fn from_buffers(buf0: [T; N], buf1: [T; N]) -> Self {
        Self {
            buffer: [buf0, buf1],
            active_half: 0,
        }
    }

    /// Get reference to both half buffers.
    pub fn buffers(&self) -> &[[T; N]; 2] {
        &self.buffer
    }

    /// Get mutable reference to both half buffers.
    pub fn buffers_mut(&mut self) -> &mut [[T; N]; 2] {
        &mut self.buffer
    }

    /// Get current active half-buffer slice to fill with new samples.
    pub fn current_half_mut(&mut self) -> &mut [T; N] {
        &mut self.buffer[self.active_half]
    }

    /// Get current active half-buffer slice.
    pub fn current_half(&self) -> &[T; N] {
        &self.buffer[self.active_half]
    }

    /// Swap active half-buffer index and return mutable slice for the next batch.
    pub fn swap_and_get_next(&mut self) -> &mut [T; N] {
        self.active_half ^= 1;
        self.current_half_mut()
    }

    /// Fill the inactive buffer half using `fill_fn`, then swap to make it active and return its slice.
    pub fn fill_and_swap<F>(&mut self, mut fill_fn: F) -> &mut [T; N]
    where
        F: FnMut(&mut [T; N]),
    {
        let next_half = self.active_half ^ 1;
        fill_fn(&mut self.buffer[next_half]);
        self.active_half = next_half;
        &mut self.buffer[next_half]
    }
}

impl<T: Copy + Default, const N: usize> Default for DmaDoubleBuffer<T, N> {
    fn default() -> Self {
        Self::new()
    }
}
