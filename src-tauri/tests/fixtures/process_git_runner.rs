use std::{
    env,
    io::{self, Write},
    process::{self, Command},
    thread,
    time::Duration,
};

const LARGE_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

fn main() {
    match env::args().nth(1).as_deref() {
        Some("print") => print!("git-output"),
        Some("sleep") => thread::sleep(Duration::from_secs(1)),
        Some("spawn-sleeper-then-mark") => {
            Command::new(env::current_exe().expect("fixture executable is available"))
                .arg("sleep-briefly")
                .status()
                .expect("fixture sleeper starts");
            std::fs::write("completed", "completed").expect("completion marker is written");
        }
        Some("sleep-briefly") => thread::sleep(Duration::from_millis(400)),
        Some("large-output") => {
            let bytes = vec![0; LARGE_OUTPUT_BYTES];
            io::stdout()
                .lock()
                .write_all(&bytes)
                .expect("fixture stdout is writable");
            io::stderr()
                .lock()
                .write_all(&bytes)
                .expect("fixture stderr is writable");
        }
        Some("fail") => {
            io::stderr()
                .lock()
                .write_all(b"failure")
                .expect("fixture stderr is writable");
            process::exit(7);
        }
        mode => panic!("unsupported process fixture mode: {mode:?}"),
    }
}
