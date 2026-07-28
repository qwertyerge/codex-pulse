use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();

    for line in stdin.lock().lines() {
        let line = line.expect("fixture stdin is readable");
        if line.contains(r#""method":"initialize""#) {
            writeln!(stdout, r#"{{"id":1,"result":{{}}}}"#).unwrap();
            stdout.flush().unwrap();
        } else if line.contains(r#""method":"account/rateLimits/read""#) {
            writeln!(
                stdout,
                r#"{{"id":2,"result":{{"rateLimitsByLimitId":{{"codex":{{"limitId":"codex","primary":{{"usedPercent":5,"windowDurationMins":10080,"resetsAt":1785814394}}}}}}}}}}"#
            )
            .unwrap();
            writeln!(
                stdout,
                r#"{{"method":"account/rateLimits/updated","params":{{"rateLimits":{{"limitId":"codex","primary":{{"usedPercent":6,"windowDurationMins":10080,"resetsAt":1785814394}}}}}}}}"#
            )
            .unwrap();
            stdout.flush().unwrap();
        }
    }
}
