use super::{detect_oom_shutdown, oom_kill_line_matches_pid, oom_shutdown_message};

#[test]
fn matches_common_kernel_oom_victim_lines_for_pid() {
    let lines = [
        "[123.456] Out of memory: Killed process 4242 (warp) total-vm:1000kB",
        "[123.456] oom-kill:constraint=CONSTRAINT_MEMCG,task=oz,pid=4242,uid=1000",
        "[123.456] oom-kill: task=oz-dev, pid=4242, uid=1000",
    ];

    for line in lines {
        assert!(oom_kill_line_matches_pid(line, 4242), "{line}");
    }
}

#[test]
fn rejects_unrelated_or_ambiguous_pid_mentions() {
    let lines = [
        "[123.456] agent pid=4242 exited",
        "[123.456] invoked oom-killer: gfp_mask=0x0",
        "[123.456] Out of memory: Killed process 42420 (warp) total-vm:1000kB",
        "[123.456] oom-kill:constraint=CONSTRAINT_MEMCG,task=oz,pid=42420,uid=1000",
        "[123.456] oom-kill:constraint=CONSTRAINT_MEMCG,task=oz,pid=4242foo,uid=1000",
        "[123.456] oom-kill:constraint=CONSTRAINT_MEMCG,task=oz,cpid=4242,uid=1000",
    ];

    for line in lines {
        assert!(!oom_kill_line_matches_pid(line, 4242), "{line}");
    }
}

#[test]
fn classifies_oom_from_exit_status_or_kernel_evidence() {
    assert_eq!(
        oom_shutdown_message(true, false).as_deref(),
        Some("agent process was OOM-killed (exit status 137)")
    );
    assert_eq!(
        oom_shutdown_message(false, true).as_deref(),
        Some("agent process was OOM-killed (kernel evidence)")
    );
    assert_eq!(
        oom_shutdown_message(true, true).as_deref(),
        Some("agent process was OOM-killed (exit status 137 and kernel evidence)")
    );
    assert!(oom_shutdown_message(false, false).is_none());
}

#[test]
fn skips_oom_detection_without_a_nonzero_exit() {
    assert!(detect_oom_shutdown(Some(0), Some(4242)).is_none());
    assert!(detect_oom_shutdown(None, Some(4242)).is_none());
}
