use cli_command_kit::Mode;
use pge_cost_cli::*;
use std::{
    ffi::OsString,
    io::{self, Write},
};
use tokio_util::sync::CancellationToken;

async fn run(args: &[&str], mode: Mode) -> (u8, String, String) {
    let mut output = Vec::new();
    let mut errors = Vec::new();
    let status = entry(
        args.iter().map(OsString::from),
        &mut output,
        &mut errors,
        mode,
        CancellationToken::new(),
    )
    .await;
    (
        status,
        String::from_utf8(output).unwrap(),
        String::from_utf8(errors).unwrap(),
    )
}

#[tokio::test]
async fn calculates_in_both_modes_and_preserves_root_flags_and_aliases() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("absent.json");
    let path = db.to_str().unwrap();
    for prefix in [vec!["pge-cost"], vec!["pge-cost", "cost", "estimate"]] {
        for mode in [Mode::Human, Mode::Agent] {
            let mut args = prefix.clone();
            args.extend([
                "--watts", "150", "--plan", " touc ", "--db", path, "--year", "2024",
            ]);
            let (status, out, err) = run(&args, mode).await;
            assert_eq!(status, 0, "{err}");
            assert!(err.is_empty());
            if mode == Mode::Agent {
                let value: serde_json::Value = serde_json::from_str(&out).unwrap();
                assert_eq!(value["plan_id"], "E-TOU-C");
                assert_eq!(value["annual_hours"], 8784);
                assert_eq!(value["embedded"], true);
                assert_eq!(value["year"], 2024);
                assert!((value["daily_energy_kwh"].as_f64().unwrap() - 3.6).abs() < 1e-12);
            } else {
                assert!(out.contains('╭'));
                assert!(out.contains("Pacific time"));
                assert!(out.contains("embedded"));
            }
        }
    }
    std::fs::write(&db, model::DEFAULT_RATES).unwrap();
    let (status, out, err) = run(
        &[
            "pge-cost", "--watts", "1000", "--plan", "e1", "--tier", "1", "--db", path,
        ],
        Mode::Agent,
    )
    .await;
    assert_eq!(status, 0, "{err}");
    let value: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(value["effective_rate"], 0.32561);
    assert_eq!(value["embedded"], false);
    std::fs::write(&db, "{}").unwrap();
    let (status, out, err) = run(
        &["pge-cost", "--watts", "1", "--plan", "e1", "--db", path],
        Mode::Agent,
    )
    .await;
    assert_eq!(status, 1);
    assert!(out.is_empty());
    assert_eq!(err.matches("ERR:").count(), 1);
}

#[tokio::test]
async fn rejects_invalid_arguments_before_files_and_emits_errors_once() {
    for args in [
        vec!["pge-cost"],
        vec!["pge-cost", "--watts", "NaN", "--plan", "E1"],
        vec!["pge-cost", "--watts", "0", "--plan", "E1"],
        vec!["pge-cost", "--watts", "1", "--plan", "unknown"],
        vec!["pge-cost", "--watts", "1", "--plan", "E1", "--tier", "3"],
        vec!["pge-cost", "--watts", "1", "--plan", "E1", "--year", "0"],
        vec!["pge-cost", "rates", "fetch", "--url", "file:///etc/passwd"],
        vec![
            "pge-cost",
            "rates",
            "fetch",
            "--url",
            "http://name:secret@localhost",
        ],
        vec!["pge-cost", "skill", "extra"],
    ] {
        let (status, output, errors) = run(&args, Mode::Agent).await;
        assert_eq!(status, 1, "{args:?}");
        assert!(output.is_empty());
        assert!(!errors.is_empty());
        assert!(!errors.contains("reading rates"));
        assert!(!errors.contains("secret"));
    }
}

#[tokio::test]
async fn metadata_helps_and_output_failures_use_the_shared_contract() {
    let expected = help_options().skill.unwrap();
    for mode in [Mode::Human, Mode::Agent] {
        let (status, output, errors) = run(&["pge-cost", "skill"], mode).await;
        assert_eq!(status, 0);
        assert_eq!(output, expected);
        assert!(errors.is_empty());
        assert_eq!(
            run(&["pge-cost", "--version"], mode).await,
            (0, "0.1.0\n".into(), String::new())
        );
        for args in [
            vec!["pge-cost", "--help"],
            vec!["pge-cost", "cost", "--help"],
            vec!["pge-cost", "rates", "fetch", "--help"],
            vec!["pge-cost", "help", "cost", "estimate"],
        ] {
            let (status, output, errors) = run(&args, mode).await;
            assert_eq!(status, 0, "{errors}");
            assert!(errors.is_empty());
            assert_eq!(output.starts_with("ALERT:"), mode == Mode::Agent);
        }
    }
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    for args in [
        vec!["pge-cost", "skill"],
        vec!["pge-cost", "--help"],
        vec!["pge-cost", "--version"],
        vec!["pge-cost", "--watts", "1", "--plan", "E1"],
    ] {
        let mut errors = Vec::new();
        assert_eq!(
            entry(
                args.into_iter().map(OsString::from),
                &mut Broken,
                &mut errors,
                Mode::Agent,
                CancellationToken::new()
            )
            .await,
            1
        );
        assert_eq!(
            String::from_utf8(errors).unwrap().matches("ERR:").count(),
            1
        );
    }
}

#[tokio::test]
async fn native_executable_has_raw_version_agent_json_and_failure_status() {
    let exe = env!("CARGO_BIN_EXE_pge-cost");
    let output = std::process::Command::new(exe)
        .arg("--version")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"0.1.0\n");
    assert!(output.stderr.is_empty());
    let output = std::process::Command::new(exe)
        .env("AGENT", "yes")
        .args(["cost", "estimate", "--watts", "150", "--plan", "EV2"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()["annual_cost"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    let output = std::process::Command::new(exe)
        .args(["--watts", "inf", "--plan", "EV2"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
}

#[test]
fn usage_skill_covers_the_live_interface_and_matches_its_install_identity() {
    let document = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/skills/pge-cost/SKILL.md"
    ))
    .unwrap();
    let options = help_options();
    let missing = cli_command_kit::skill_omissions(&command(), &options, &document);
    assert!(missing.is_empty(), "{missing:?}");
    let pointer = options.skill.unwrap();
    assert!(pointer.contains("--skill pge-cost\n"));
    assert!(pointer.contains("/skills/pge-cost\n"));
    assert!(document.lines().any(|line| line == "name: pge-cost"));
}
