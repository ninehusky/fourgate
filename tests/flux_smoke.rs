//! CI fixture validation, not a per-body coverage check.
use serde_json::Value;
use std::{fs, path::PathBuf};

type Error = Box<dyn std::error::Error>;

#[test]
#[ignore = "requires captured output from pinned Flux; CI sets FOURGATE_SMOKE_DIR"]
fn pinned_flux_assertion() -> Result<(), Error> {
    let directory = PathBuf::from(std::env::var("FOURGATE_SMOKE_DIR")?);
    let logs = [
        fs::read_to_string(directory.join("stdout.log"))?,
        fs::read_to_string(directory.join("stderr.log"))?,
    ];
    let result: Value = serde_json::from_slice(&fs::read(directory.join("result.json"))?)?;
    validate([&logs[0], &logs[1]], &result)?;
    println!("verified expected assertion diagnostic at src/lib.rs:2");
    Ok(())
}

fn validate(logs: [&str; 2], result: &Value) -> Result<(), Error> {
    // An ICE makes diagnostic coverage inconclusive, even if the assertion appeared.
    if logs.iter().any(|log| {
        log.contains("panicked") || log.to_lowercase().contains("internal compiler error")
    }) {
        return Err("Flux panicked; smoke evidence is inconclusive".into());
    }
    if !matches!(result["exit_code"].as_i64(), Some(code) if code != 0)
        || !result["launch_error"].is_null()
    {
        return Err("expected a completed failing invocation".into());
    }
    let found = logs.iter().flat_map(|log| log.lines()).any(|line| {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            return false;
        };
        let diagnostic = &event["message"];
        diagnostic["code"]["code"] == "E0999"
            && diagnostic["message"]
                .as_str()
                .is_some_and(|msg| msg.contains("refinement type error"))
            && diagnostic["spans"].as_array().is_some_and(|spans| {
                spans.iter().any(|span| {
                    span["is_primary"] == true
                        && span["line_start"] == 2
                        && span["file_name"].as_str().is_some_and(|file| {
                            file == "src/lib.rs" || file.ends_with("/src/lib.rs")
                        })
                })
            })
    });
    if !found {
        return Err("expected assertion diagnostic absent; inspect raw logs".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn diagnostic(line: u32) -> String {
        json!({"message": {"code":{"code":"E0999"}, "message":"refinement type error",
            "spans":[{"is_primary":true,"line_start":line,"file_name":"src/lib.rs"}]}})
        .to_string()
    }

    #[test]
    fn accepts_specific_assertion_among_other_errors() {
        let log = format!("{}\n{}", diagnostic(7), diagnostic(2));
        assert!(validate([&log, ""], &json!({"exit_code":101})).is_ok());
    }

    #[test]
    fn unrelated_error_is_not_evidence() {
        assert!(validate([&diagnostic(7), ""], &json!({"exit_code":101})).is_err());
    }

    #[test]
    fn panic_invalidates_matching_diagnostic() {
        assert!(
            validate(
                [&diagnostic(2), "thread 'rustc' panicked"],
                &json!({"exit_code":101})
            )
            .is_err()
        );
    }

    #[test]
    fn success_or_launch_failure_is_not_expected_failure() {
        for result in [
            json!({"exit_code":0}),
            json!({"exit_code":null,"launch_error":"not found"}),
        ] {
            assert!(validate([&diagnostic(2), ""], &result).is_err());
        }
    }
}
