use crate::GlobalConfig;
use std::{
    sync::mpsc,
    thread::{self, JoinHandle},
    process::Command,
};

pub fn trayer(global_config: &GlobalConfig, ch: mpsc::Sender<Event>) -> JoinHandle<()> {
    let mut trayer = Command::new("trayer");
    trayer.args(&[
        "--edge",
        "top",
        "--align",
        "right",
        "--widthtype",
        "request",
        "--height",
        global_config
            .base_geometry
            .as_ref()
            .and_then(|x| x.split("x").nth(1))
            .and_then(|x| x.split("+").next())
            .unwrap_or("18"),
        "--transparent",
        "true",
        "--expand",
        "true",
        "--SetDockType",
        "true",
        "--monitor",
        "primary",
        "--alpha",
        &global_config
            .background
            .as_ref()
            .and_then(Color::transparency)
            .map(|a| 255 - a)
            .unwrap_or(0)
            .to_string(),
        "--tint",
        &format!(
            "0x{}",
            global_config
                .background
                .as_ref()
                .map(|c| c.tint())
                .unwrap_or("FFFFFF")
        ),
    ]);
    thread::spawn(move || {
        let _ = Command::new("killall")
            .arg("trayer")
            .spawn()
            .and_then(|mut ch| ch.wait());
        let mut trayer = match trayer.spawn() {
            Err(e) => return log::error!("Couldn't start trayer: {}", e),
            Ok(t) => t,
        };
        thread::sleep(Duration::from_millis(500));
        let xprop = Command::new("xprop")
            .args(&[
                "-name",
                "panel",
                "-f",
                "WM_SIZE_HINTS",
                "32i",
                " $5\n",
                "-spy",
                "WM_NORMAL_HINTS",
            ])
            .stdout(Stdio::piped())
            .spawn();
        let mut xprop = match xprop {
            Err(e) => return log::error!("Couldn't spy tray size: {:?}", e),
            Ok(x) => x,
        };
        let xprop_output = match xprop.stdout.take() {
            None => return log::error!("Couldn't read tray size spy output"),
            Some(o) => o,
        };
        let _ = BufReader::new(xprop_output)
            .lines()
            .filter_map(Result::ok)
            .try_for_each(|l| {
                l.split(" ")
                    .nth(1)
                    .and_then(|x| {
                        x.parse()
                            .map_err(|_| log::error!("Failed to parse size: '{}' from '{}'", x, l))
                            .ok()
                    })
                    .and_then(|o: u32| ch.send(Event::TrayResize(o + 5)).ok())
            });
        let _ = xprop.kill();
        let _ = trayer.kill();
        let _ = xprop.wait();
        let _ = trayer.wait();
    })
}
