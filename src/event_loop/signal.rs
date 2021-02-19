extern "C" {
    pub fn signal(sig: i32, handler: extern "C" fn(i32)) -> extern "C" fn(i32);
}
