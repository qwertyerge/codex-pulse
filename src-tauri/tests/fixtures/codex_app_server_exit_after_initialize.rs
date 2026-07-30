use std::{
    env,
    fs::OpenOptions,
    io::{self, BufRead, Write},
};

fn main() {
    let completed_handshakes = env::current_exe()
        .expect("fixture executable path is available")
        .with_extension("handshakes");

    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line.expect("fixture stdin is readable");
        if line.contains(r#""method":"initialize""#) {
            writeln!(stdout, r#"{{"id":1,"result":{{}}}}"#).unwrap();
            stdout.flush().unwrap();
        } else if line.contains(r#""method":"account/rateLimits/read""#) {
            writeln!(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(completed_handshakes)
                    .expect("fixture handshake log opens"),
                "completed"
            )
            .expect("completed handshake is recorded");
            return;
        }
    }
}
