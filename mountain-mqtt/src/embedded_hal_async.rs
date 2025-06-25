use crate::client::Delay;

pub struct DelayEmbedded<T>
where
    T: embedded_hal_async::delay::DelayNs,
{
    inner: T,
}

impl<T> DelayEmbedded<T>
where
    T: embedded_hal_async::delay::DelayNs,
{
    /// Create a new adapter
    pub fn new(inner: T) -> Self {
        DelayEmbedded { inner }
    }

    /// Consume the adapter, returning the inner object.
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Borrow the inner object.
    pub fn inner(&self) -> &T {
        &self.inner
    }

    /// Mutably borrow the inner object.
    pub fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T> Delay for DelayEmbedded<T>
where
    T: embedded_hal_async::delay::DelayNs,
{
    async fn delay_us(&mut self, us: u32) {
        self.inner.delay_us(us).await
    }
}
