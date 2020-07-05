pub trait NewCustom : Sized {
	fn new_custom<F: FnOnce() -> Self>(setup: F) -> Self;
}

impl<T: Sized> NewCustom for T {
	fn new_custom<F: FnOnce() -> Self>(setup: F) -> Self {
		setup()
	}
}

