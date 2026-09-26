use super::*;
use std::io::Write;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Instant;

const MODE: &str = "GATEWAY_ENCODER_PROCESS_TEST_MODE";
const DIRECTORY: &str = "GATEWAY_ENCODER_PROCESS_TEST_DIRECTORY";

#[tokio::test]
async fn gemini_canvas_encoder_process_admission_timeout_and_drop_restore_capacity() {
    let slots = Arc::new(Semaphore::new(1));
    let admission = EncoderAdmission::acquire_from(slots.clone(), Duration::from_secs(5))
        .await
        .unwrap();
    assert_eq!(slots.available_permits(), 0);
    assert!(
        EncoderAdmission::acquire_from(slots.clone(), Duration::from_millis(20))
            .await
            .is_none()
    );
    drop(admission);
    assert_eq!(slots.available_permits(), 1);
    assert!(
        EncoderAdmission::acquire_from(slots.clone(), Duration::ZERO)
            .await
            .is_none()
    );
    assert_eq!(slots.available_permits(), 1);
}

#[tokio::test]
async fn gemini_canvas_encoder_process_expired_preparation_does_not_spawn() {
    let fixture = Fixture::new();
    let slots = Arc::new(Semaphore::new(1));
    let admission = EncoderAdmission::acquire_from(slots.clone(), Duration::from_secs(5))
        .await
        .unwrap();
    let expired = EncoderAdmission {
        deadline: tokio::time::Instant::now(),
        ..admission
    };
    assert!(expired.run(fixture.command("sleep")).await.is_none());
    assert!(!fixture.0.join("ready").exists());
    assert_eq!(slots.available_permits(), 1);
}

#[tokio::test]
async fn gemini_canvas_encoder_process_cancelled_queue_restores_admission() {
    let slots = Arc::new(Semaphore::new(1));
    let admission = EncoderAdmission::acquire_from(slots.clone(), Duration::from_secs(5))
        .await
        .unwrap();
    let waiting_slots = slots.clone();
    let (polled, ready) = oneshot::channel();
    let waiter = tokio::spawn(async move {
        let mut waiting = Box::pin(EncoderAdmission::acquire_from(
            waiting_slots,
            Duration::from_secs(5),
        ));
        // Poll once to prove the acquisition is queued, rather than aborting an unstarted task.
        std::future::poll_fn(|cx| {
            assert!(std::future::Future::poll(waiting.as_mut(), cx).is_pending());
            std::task::Poll::Ready(())
        })
        .await;
        polled.send(()).unwrap();
        waiting.await
    });
    ready.await.unwrap();
    waiter.abort();
    assert!(waiter.await.is_err_and(|error| error.is_cancelled()));
    assert_eq!(slots.available_permits(), 0);
    drop(admission);
    let next = EncoderAdmission::acquire_from(slots.clone(), Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(slots.available_permits(), 0);
    drop(next);
    assert_eq!(slots.available_permits(), 1);
}

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let parent = std::env::temp_dir().canonicalize().unwrap();
        let path = parent.join(format!(
            "gateway-encoder-process-test-{}",
            uuid::Uuid::new_v4()
        ));
        assert_eq!(path.parent(), Some(parent.as_path()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn command(&self, mode: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", &probe_name(), "--ignored", "--nocapture"])
            .env(MODE, mode)
            .env(DIRECTORY, &self.0)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn probe_name() -> String {
    format!(
        "{}::encoder_process_probe",
        module_path!().split_once("::").unwrap().1
    )
}

#[test]
#[ignore = "executed only by isolated encoder process parent tests"]
fn encoder_process_probe() {
    let directory = PathBuf::from(std::env::var_os(DIRECTORY).unwrap());
    match std::env::var(MODE).unwrap().as_str() {
        "output" => {
            print!("encoder-stdout");
            eprint!("encoder-stderr");
        }
        "nonzero" => {
            eprintln!("encoder-nonzero");
            std::process::exit(23);
        }
        "oversized" => {
            std::io::stdout()
                .write_all(&vec![b'x'; 128 * 1024])
                .unwrap();
        }
        "sleep" | "leaf" => {
            std::fs::write(directory.join("ready"), b"ready").unwrap();
            std::thread::sleep(Duration::from_secs(2));
            std::fs::write(directory.join("late"), b"unexpected").unwrap();
        }
        "tree" => {
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", &probe_name(), "--ignored", "--nocapture"])
                .env(MODE, "leaf")
                .env(DIRECTORY, &directory)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let _ = child.wait();
        }
        mode => panic!("unknown encoder test mode: {mode}"),
    }
}

#[tokio::test]
async fn gemini_canvas_encoder_process_preserves_output_and_nonzero_status() {
    let fixture = Fixture::new();
    let output = run_encoder_process(fixture.command("output"), Duration::from_secs(5))
        .await
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("encoder-stdout"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("encoder-stderr"));
    let output = run_encoder_process(fixture.command("nonzero"), Duration::from_secs(5))
        .await
        .unwrap();
    assert_eq!(output.status.code(), Some(23));
    assert!(String::from_utf8_lossy(&output.stderr).contains("encoder-nonzero"));
}

#[tokio::test]
async fn gemini_canvas_encoder_process_rejects_excess_output() {
    let fixture = Fixture::new();
    assert!(
        run_encoder_process(fixture.command("oversized"), Duration::from_secs(5))
            .await
            .is_none()
    );
}

#[tokio::test]
async fn gemini_canvas_encoder_process_timeout_stops_descendant() {
    let fixture = Fixture::new();
    let slots = Arc::new(Semaphore::new(1));
    let permit = Arc::new(slots.clone().try_acquire_owned().unwrap());
    let (_cancel, cancellation) = oneshot::channel();
    let (expire, expired) = oneshot::channel();
    // Arm expiry only after the real descendant starts; OS startup time is not the assertion.
    let task = tokio::spawn(supervise(
        fixture.command("tree"),
        tokio::time::Instant::now() + Duration::from_secs(5),
        cancellation,
        permit,
        Arc::new(()),
        async move {
            expired.await.unwrap();
        },
    ));
    let startup_deadline = Instant::now() + Duration::from_secs(5);
    while !fixture.0.join("ready").exists() {
        assert!(
            Instant::now() < startup_deadline,
            "descendant fixture never started"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    expire.send(()).unwrap();
    assert!(tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap()
        .is_none());
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert!(
        !fixture.0.join("late").exists(),
        "descendant survived timeout"
    );
    assert_eq!(slots.available_permits(), 1);
}

#[tokio::test]
async fn gemini_canvas_encoder_process_cancellation_stops_started_child() {
    let fixture = Fixture::new();
    let command = fixture.command("sleep");
    let slots = Arc::new(Semaphore::new(1));
    let admission = EncoderAdmission::acquire_from(slots.clone(), Duration::from_secs(10))
        .await
        .unwrap();
    let workspace = Arc::new(
        crate::upstream::gemini_canvas_encoder_workspace::EncoderWorkspace::create(
            b"source",
            "png",
            admission.lease(),
        )
        .unwrap(),
    );
    let source = workspace.source.clone();
    let weak_workspace = Arc::downgrade(&workspace);
    let task = tokio::spawn(admission.run_retaining(command, workspace));
    let deadline = Instant::now() + Duration::from_secs(5);
    while !fixture.0.join("ready").exists() {
        assert!(Instant::now() < deadline, "child never started");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(source.exists());
    assert_eq!(slots.available_permits(), 0);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::sleep(Duration::from_millis(2500)).await;
    assert!(
        !fixture.0.join("late").exists(),
        "child survived cancellation"
    );
    assert!(weak_workspace.upgrade().is_none());
    assert!(!source.parent().unwrap().exists());
    assert_eq!(slots.available_permits(), 1);
}
