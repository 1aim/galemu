use std::ops::Not;

pub fn not(val: impl Not<Output=bool>) -> bool {
    val.not()
}