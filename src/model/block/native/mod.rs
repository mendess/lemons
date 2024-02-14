mod clock;
#[cfg(feature = "hyprland")]
mod hyprland;
mod music;

pub fn new(cmd: &'static str) -> Option<Box<dyn super::BlockTask>> {
    match cmd {
        "m" => Some(Box::new(music::Music)),
        "clock" => Some(Box::new(clock::Clock)),
        #[cfg(feature = "hyprland")]
        "hyprland" => Some(Box::new(hyprland::HyprLand)),
        _ => None,
    }
}
