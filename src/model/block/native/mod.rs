mod clock;
#[cfg(feature = "hyprland")]
mod hyprland;
mod music;

pub mod native_block {
    pub const M: &str = "m";
    pub const CLOCK: &str = "clock";
    #[cfg(feature = "hyprland")]
    pub const HYPRLAND: &str = "hyprland";
}

pub fn new(cmd: &'static str) -> Option<Box<dyn super::BlockTask>> {
    match cmd {
        native_block::M => Some(Box::new(music::Music)),
        native_block::CLOCK => Some(Box::new(clock::Clock)),
        #[cfg(feature = "hyprland")]
        native_block::HYPRLAND => Some(Box::new(hyprland::HyprLand)),
        _ => None,
    }
}
