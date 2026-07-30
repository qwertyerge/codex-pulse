use std::{
    env,
    fs::OpenOptions,
    io::{self, BufRead, Write},
    time::{SystemTime, UNIX_EPOCH},
};

fn main() {
    let starts = env::current_exe()
        .expect("fixture executable path is available")
        .with_extension("starts");
    let started_at_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time follows the Unix epoch")
        .as_millis();
    writeln!(
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(starts)
            .expect("fixture start log opens"),
        "{started_at_ms}"
    )
    .expect("fixture start is recorded");

    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line.expect("fixture stdin is readable");
        if line.contains(r#""method":"initialize""#) {
            writeln!(stdout, r#"{{"id":1,"result":{{}}}}"#).unwrap();
            stdout.flush().unwrap();
        } else if line.contains(r#""method":"account/rateLimits/read""#) {
            return;
        }
    }
}
