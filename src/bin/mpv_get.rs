use std::{env::args, time::Duration};
use tokio::{
    io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
    time,
};

fn query(s: &str) -> Vec<u8> {
    let mut v = serde_json::to_vec(&serde_json::json!({
        "command": [ "get_property", s.trim_end() ]
    }))
    .unwrap();
    v.push(b'\n');
    v
}

async fn client(s: &str) -> io::Result<()> {
    let mut stdin = BufReader::new(io::stdin());
    let mut socket = UnixStream::connect(s).await?;
    let mut buf = String::new();
    let mut server_buf = Vec::new();
    loop {
        buf.clear();
        eprint!("$ ");
        if 0 == stdin.read_line(&mut buf).await? {
            break Ok(());
        }
        eprintln!("wait to write");
        socket.writable().await?;
        let q = query(&buf);
        eprintln!("Write: {:?}", std::str::from_utf8(&q).unwrap());
        let n = socket.write(&q).await?;
        eprintln!("wait to read");
        match time::timeout(Duration::from_secs(5), socket.readable()).await {
            Ok(r) => r,
            Err(_) => {
                eprintln!("timed out");
                continue;
            }
        }?;
        server_buf.resize_with(n, Default::default);
        eprintln!("read");
        loop {
            match socket.try_read_buf(&mut server_buf) {
                Ok(r) => r,
                Err(_) => {
                    eprintln!("failed");
                    break;
                }
            };
        }
        if let Some(i) = server_buf.iter().position(|b| *b != 0) {
            println!("> {}", std::str::from_utf8(&server_buf[i..]).unwrap_or(""));
            println!("#0: {}", i);
        }
        server_buf.clear();
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    client(&args().nth(1).expect("socket name")).await
}
