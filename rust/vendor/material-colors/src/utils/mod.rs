pub mod math;
#[cfg(not(feature = "std"))] pub mod no_std;

pub trait FromRef<T> {
    fn from_ref(value: &T) -> Self;
}
