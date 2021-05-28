use std::{env::args, time::Duration};
use tokio::{
    io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{unix::SocketAddr, UnixListener, UnixStream},
    time,
};

const SOCKET: &str = "/tmp/test-socket";

async fn handle_client(s: UnixStream, addr: SocketAddr) -> io::Result<()> {
    'outer: loop {
        let mut buf = [0; 1024];
        eprintln!("{:?} test readable", addr);
        s.readable().await?;
        let n = loop {
            match s.try_read(&mut buf) {
                Ok(0) => break 'outer,
                Ok(n) => break n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => return Err(e),
            }
        };
        eprintln!("{:?} read {} bytes", addr, n);
        s.writable().await?;
        eprintln!("{:?} writing back", addr);
        match s.try_write(&buf[..n]) {
            Ok(n) => eprintln!("wrote {} bytes", n),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => (),
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

async fn server() -> io::Result<()> {
    let _ = std::fs::remove_file(SOCKET);
    let socket = UnixListener::bind(SOCKET)?;
    while let Ok(s) = socket.accept().await {
        tokio::spawn(async move {
            if let Err(e) = handle_client(s.0, s.1).await {
                eprintln!("exited: {}", e);
            } else {
                eprintln!("client closed");
            }
        });
    }
    Ok(())
}

async fn client() -> io::Result<()> {
    let mut stdin = BufReader::new(io::stdin());
    let mut buf = String::new();
    eprint!("socket: ");
    stdin.read_line(&mut buf).await?;
    buf.pop();

    let mut socket = UnixStream::connect(if buf.is_empty() { SOCKET } else { &buf }).await?;
    let mut server_buf = Vec::new();
    loop {
        buf.clear();
        eprint!("$ ");
        if 0 == stdin.read_line(&mut buf).await? {
            break Ok(());
        }
        eprintln!("wait to write");
        socket.writable().await?;
        eprintln!("Write: {:?}", buf);
        let n = socket.write(buf.as_bytes()).await?;
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
    if args().count() > 1 {
        server().await
    } else {
        client().await
    }
}
