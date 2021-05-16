mod music;

pub fn new(cmd: &'static str) -> Option<Box<dyn super::BlockTask>> {
    match cmd {
        "m" => Some(Box::new(music::Music)),
        _ => None
    }
}
