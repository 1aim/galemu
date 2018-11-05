

#[doc(hidden)]
#[macro_export]
macro_rules! unsafe_block {
    ($why:tt => $code:block) => ({
        #[allow(unsafe_code)]
        unsafe {
            $code
        }
    });
}