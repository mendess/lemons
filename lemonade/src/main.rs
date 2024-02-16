use std::io::{self, BufRead, BufReader};

fn main() -> io::Result<()> {
    let mut buf = String::new();
    let mut reader = BufReader::new(std::io::stdin().lock());
    loop {
        buf.clear();
        reader.read_line(&mut buf)?;
        match buf.pop() {
            Some('\n') => {}
            Some(other) => panic!("expected line to be terminated by newline, instead got {other}"),
            None => break Ok(()),
        }
        eprintln!("======================= NEW LINE =======================");
        eprintln!("{buf}");
        for b in lemonade::parser::parse(&buf) {
            let b = b.map_err(io::Error::other)?;
            eprintln!("{b:?}");
        }
    }
}
