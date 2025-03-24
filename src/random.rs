pub trait Random {
    fn random(rng: esp_hal::rng::Rng) -> Self;
}

impl<T: bytemuck::NoUninit + bytemuck::AnyBitPattern> Random for T {
    fn random(mut rng: esp_hal::rng::Rng) -> Self {
        let mut buf: Self = bytemuck::Zeroable::zeroed();
        rng.read(bytemuck::bytes_of_mut(&mut buf));
        buf
    }
}
