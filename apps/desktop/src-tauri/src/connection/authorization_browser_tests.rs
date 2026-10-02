use super::*;

const AUTHORIZATION: &str =
    "https://auth.openai.com/oauth/authorize?state=test&code_challenge=test";

#[test]
fn allows_only_the_exact_https_authorization_endpoint() {
    assert!(allowed(&Url::parse(AUTHORIZATION).unwrap()));
    assert!(allowed(
        &Url::parse("https://auth.openai.com:443/oauth/authorize").unwrap()
    ));
    for value in [
        "http://auth.openai.com/oauth/authorize",
        "https://auth.openai.com:444/oauth/authorize",
        "https://auth.openai.com.evil.example/oauth/authorize",
        "https://auth.openai.com./oauth/authorize",
        "https://user:secret@auth.openai.com/oauth/authorize",
        "https://auth.openai.com/oauth/token",
        "https://auth.openai.com/oauth/%61uthorize",
        "https://auth.openai.com/oauth/authorize#fragment",
        "file:///tmp/oauth/authorize",
        "javascript:alert(1)",
    ] {
        assert!(!allowed(&Url::parse(value).unwrap()), "{value}");
    }
    let long = format!("{AUTHORIZATION}&extra={}", "x".repeat(8192));
    assert!(!allowed(&Url::parse(&long).unwrap()));
}

#[test]
fn rejects_overlap_and_joins_the_worker_on_exit() {
    let browser = AuthorizationBrowser::default();
    let (sent, received) = std::sync::mpsc::channel();
    assert!(browser.spawn(move |stopping| {
        while !stopping.load(Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(5));
        }
        sent.send(()).unwrap();
    }));
    assert!(!browser.spawn(|_| panic!("overlapping launch")));
    browser.stop();
    received.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(browser.launch.lock().unwrap().worker.is_none());
    assert!(!browser.spawn(|_| panic!("launch after exit")));
}

#[test]
fn passes_the_url_as_one_argument_without_a_shell() {
    let url = Url::parse(AUTHORIZATION).unwrap();
    let command = browser_command(&url);
    let args: Vec<_> = command.get_args().collect();
    assert_eq!(args.last().unwrap(), &url.as_str());
    #[cfg(target_os = "windows")]
    {
        assert_eq!(command.get_program(), "rundll32.exe");
        assert_eq!(args.len(), 2);
        assert_eq!(args[0], "url.dll,FileProtocolHandler");
    }
    #[cfg(not(target_os = "windows"))]
    assert_eq!(args.len(), 1);
}

#[cfg(target_os = "windows")]
fn child(script: &str) -> Command {
    use std::os::windows::process::CommandExt;
    let mut command = Command::new("powershell.exe");
    command.args(["-NoProfile", "-NonInteractive", "-Command", script]);
    command.creation_flags(0x08000000);
    command
}

#[cfg(target_os = "windows")]
#[test]
fn waits_for_success_and_reports_process_failure() {
    let stopping = AtomicBool::new(false);
    assert!(run_bounded(
        child("exit 0"),
        Duration::from_secs(5),
        &stopping
    ));
    assert!(!run_bounded(
        child("exit 7"),
        Duration::from_secs(5),
        &stopping
    ));
}

#[cfg(target_os = "windows")]
#[test]
fn kills_and_reaps_its_child_at_the_deadline() {
    let started = Instant::now();
    assert!(!run_bounded(
        child("Start-Sleep -Seconds 30"),
        Duration::from_millis(100),
        &AtomicBool::new(false)
    ));
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[cfg(target_os = "windows")]
#[test]
fn application_exit_cancels_a_live_launcher() {
    let browser = AuthorizationBrowser::default();
    assert!(browser.spawn(|stopping| {
        assert!(!run_bounded(
            child("Start-Sleep -Seconds 30"),
            Duration::from_secs(10),
            &stopping
        ));
    }));
    std::thread::sleep(Duration::from_millis(200));
    let started = Instant::now();
    browser.stop();
    assert!(started.elapsed() < Duration::from_secs(5));
}
