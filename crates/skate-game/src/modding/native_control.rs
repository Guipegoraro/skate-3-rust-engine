//! A native setting one mod at a time may own (trainer, gravity, ...). The owner's value
//! applies until it retires, fails or is disabled; then the stock default returns.
pub(crate) struct NativeControl<T> {
    name: &'static str,
    owner: Option<(String, T)>,
}
impl<T: Copy + Default> NativeControl<T> {
    pub const fn new(name: &'static str) -> Self {
        Self { name, owner: None }
    }
    /// Current value, stock default when unowned.
    pub fn value(&self) -> T {
        self.owner.as_ref().map(|(_, v)| *v).unwrap_or_default()
    }
    pub fn claim(&mut self, id: &str, value: T) -> Result<(), String> {
        if self.owner.as_ref().is_some_and(|(owner, _)| owner != id) {
            return Err(format!("Native {} controls are already owned by another mod", self.name));
        }
        self.owner = Some((id.to_owned(), value));
        Ok(())
    }
    /// Drops ownership when `id` owns it; returns whether it did.
    pub fn release(&mut self, id: &str) -> bool {
        let owned = self.owner.as_ref().is_some_and(|(owner, _)| owner == id);
        if owned {
            self.owner = None;
        }
        owned
    }
}

/// Visual skater size; the default is stock size.
#[derive(Clone, Copy)]
pub(crate) struct SkaterScale(pub f32);
impl Default for SkaterScale {
    fn default() -> Self {
        Self(1.0)
    }
}

/// Gravity multiplier; the default is stock gravity.
#[derive(Clone, Copy)]
pub(crate) struct Gravity(pub f32);
impl Default for Gravity {
    fn default() -> Self {
        Self(1.0)
    }
}
