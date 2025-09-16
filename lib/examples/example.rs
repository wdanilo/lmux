use tokio::time::sleep;
use tokio::time::Duration;

// =================
// === Constants ===
// =================

const WAIT_FOR_TASKS: bool = true;
const DEBUG_SLOW: bool = false;
const LINE_DELAY: u64 = if DEBUG_SLOW { 100 } else { 10 };
const START_DELAY: u64 = if DEBUG_SLOW { 1000 } else { 100 };

// =================
// === Mock Data ===
// =================

struct TaskConfig {
    start_delay: u64,
    lines: usize,
    line_delay: u64,
    line_status: Box<dyn Fn(&TaskConfig, usize) -> lmux::Status + Send + Sync + 'static>,
}

fn tasks() -> Vec<TaskConfig> {
    vec![
        TaskConfig {
            start_delay: 0 * START_DELAY,
            lines: 100,
            line_delay: LINE_DELAY,
            line_status: Box::new(|cfg, line| {
                if line != cfg.lines { lmux::Status::ok() }
                else                 { lmux::Status::ok().finished() }
            }),
        },
        TaskConfig {
            start_delay: 1 * START_DELAY,
            lines: 100,
            line_delay: LINE_DELAY,
            line_status: Box::new(|cfg, line| {
                if line != cfg.lines { lmux::Status::ok().progress(line as f32 / cfg.lines as f32) }
                else                 { lmux::Status::ok().finished() }
            }),
        },
        TaskConfig {
            start_delay: 2 * START_DELAY,
            lines: 100,
            line_delay: LINE_DELAY,
            line_status: Box::new(|cfg, line| {
                if line != cfg.lines { lmux::Status::ok().progress(line as f32 / cfg.lines as f32) }
                else                 { lmux::Status::ok().finished() }
            }),
        },
        TaskConfig {
            start_delay: 3 * START_DELAY,
            lines: 80,
            line_delay: LINE_DELAY,
            line_status: Box::new(|cfg, line| {
                if line != cfg.lines { lmux::Status::ok().progress(line as f32 / 100.0) }
                else                 { lmux::Status::error().progress(0.8).finished() }
            }),
        },
        TaskConfig {
            start_delay: 4 * START_DELAY,
            lines: 100,
            line_delay: LINE_DELAY,
            line_status: Box::new(|cfg, line| {
                let progress = line as f32 / cfg.lines as f32;
                if line != cfg.lines { lmux::Status::ok().progress(progress) }
                else                 { lmux::Status::ok().finished() }
            }),
        },
    ]
}

// ============
// === Main ===
// ============

#[tokio::main]
async fn main() {
    let tui_handle = tokio::task::spawn_blocking(|| {
        let out = lmux::main();
        println!("Result: {out:?}")
    });

    let handles: Vec<_> = tasks().into_iter().enumerate()
        .map(|(i, cfg)| {
            let id = format!("task_{i}");
            let label = format!("TASK {i}");
            lmux::set_header(&id, label);
            tokio::spawn(async move {
                sleep(Duration::from_millis(cfg.start_delay)).await;
                for line in 1..=cfg.lines {
                    let status = (cfg.line_status)(&cfg, line);
                    lmux::log(&id, status, format!("Output line {line}"));
                    let is_last_line = line == cfg.lines;
                    if !is_last_line {
                        sleep(Duration::from_millis(cfg.line_delay)).await;
                    }
                }
            })
        })
        .collect();

    if WAIT_FOR_TASKS {
        for handle in handles {
            handle.await.unwrap();
        }
        lmux::debug("All tasks done.");
    }
    tui_handle.await.unwrap();
}
