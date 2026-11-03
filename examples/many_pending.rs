//! Load check for the background-thread auto-dismiss poll.
//!
//! Sends `NOTIFICATION_COUNT` notifications that wait for a click, each from its own thread,
//! with the main thread running the run loop (the long-running daemon pattern). It reports the
//! CPU time the process used while they sit unanswered, then removes them all from Notification
//! Centre and checks every waiting thread comes back with `NotificationResponse::None`.
//!
//! `cargo run --example many_pending`

#![allow(deprecated)]

use std::{
    process::Command,
    thread,
    time::{Duration, Instant},
};

use mac_notification_sys::*;
use objc2_foundation::{NSDate, NSRunLoop, NSUserNotificationCenter};

const NOTIFICATION_COUNT: usize = 20;
const IDLE: Duration = Duration::from_secs(20);

/// CPU time used by this process so far, in seconds, from `ps`.
fn cpu_seconds() -> f64 {
    let out = Command::new("ps")
        .args(["-o", "time=", "-p", &std::process::id().to_string()])
        .output()
        .unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    // [[hh:]mm:]ss.ss
    text.trim()
        .split(':')
        .fold(0.0, |acc, part| acc * 60.0 + part.parse::<f64>().unwrap())
}

fn run_loop_for(duration: Duration) {
    let run_loop = NSRunLoop::mainRunLoop();
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        run_loop.runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.1));
    }
}

fn main() {
    let bundle = get_bundle_identifier_or_default("safari");
    set_application(&bundle).unwrap();

    let handles: Vec<_> = (0..NOTIFICATION_COUNT)
        .map(|i| {
            thread::spawn(move || {
                Notification::new()
                    .title("many_pending example")
                    .message(&format!("Notification {}", i + 1))
                    .wait_for_click(true)
                    .send()
                    .unwrap()
            })
        })
        .collect();

    // let every notification be delivered before measuring
    run_loop_for(Duration::from_secs(3));

    let before = cpu_seconds();
    run_loop_for(IDLE);
    let used = cpu_seconds() - before;
    println!(
        "{NOTIFICATION_COUNT} pending notifications: {used:.2}s CPU over {}s ({:.1}%)",
        IDLE.as_secs(),
        used / IDLE.as_secs_f64() * 100.0
    );

    NSUserNotificationCenter::defaultUserNotificationCenter().removeAllDeliveredNotifications();
    let cleared = Instant::now();
    let run_loop = NSRunLoop::mainRunLoop();
    while !handles.iter().all(|h| h.is_finished()) {
        assert!(
            cleared.elapsed() < Duration::from_secs(10),
            "waiting threads did not resolve after removal"
        );
        run_loop.runUntilDate(&NSDate::dateWithTimeIntervalSinceNow(0.1));
    }
    let responses: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let none = responses
        .iter()
        .filter(|r| matches!(r, NotificationResponse::None))
        .count();
    println!(
        "removed: {none}/{NOTIFICATION_COUNT} resolved as None in {:.1}s",
        cleared.elapsed().as_secs_f64()
    );
}
