pub mod devices;
#[cfg(feature = "eq-support")]
pub mod eq;

#[macro_export]
macro_rules! debug_println {
    ($($args:tt)*) => {
        #[cfg(debug_assertions)]
        println!($($args)*);
    };
}
