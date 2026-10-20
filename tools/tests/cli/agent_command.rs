//! Feed real hook events to the CLI without executing any candidate commands.

use std::io::{self, Write};
use std::process::{Output, Stdio};

use serde_json::{Value, json};

fn invoke(input: &str) -> io::Result<Output> {
    let mut child = super::tool()
        .args(["checks", "agent-command"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .take()
        .ok_or_else(|| io::Error::other("missing child stdin"))?
        .write_all(input.as_bytes())?;
    child.wait_with_output()
}

fn event(tool: &str, command: &str) -> String {
    json!({
        "hook_event_name": "PreToolUse", "tool_name": tool,
        "tool_input": {"command": command}, "cwd": "C:/a different working directory"
    })
    .to_string()
}

const PROHIBITED_COMMANDS: &[&str] = &[
    "python -c 'print(1)'",
    "python3 - <<'PY'\nprint(1)\nPY",
    "py temporary.py",
    "node --eval 'console.log(1)'",
    "node -p '1 + 1'",
    "node /tmp/helper.js",
    "ruby -e 'puts 1'",
    "echo code | python",
    "Get-Content helper.py | python -",
    "cargo test && python -c 'print(1)'",
    "git status; node helper.js",
    "git status\npython helper.py",
    "& 'C:\\Program Files\\Python\\python.exe' -c 'print(1)'",
    "'C:/Program Files/nodejs/node.exe' helper.js",
    "env MODE=test python -c 'print(1)'",
    "MODE=test python helper.py",
    "command python helper.py",
    "./temporary.ps1",
    "/tmp/extensionless-helper",
    "& $env:TEMP/helper",
    "& $helper -c code",
    "if true; then python helper.py; fi",
    "& './temporary.ps1'",
    "./tools/looks-maintained.py",
    "bash -c 'python helper.py'",
    "pwsh -EncodedCommand abc",
    "cmd /c helper.cmd",
    "for file in *.rs; do echo $file; done",
    "foreach ($file in $files) { Get-Content $file }",
    "Get-ChildItem | ForEach-Object { $_.Name }",
    "Get-ChildItem | % { $_.Name }",
    "function edit { Set-Content file value }",
    "edit() { echo value; }",
    "Invoke-Expression $code",
    "echo $(python helper.py)",
    "echo `python helper.py`",
    "xargs python helper.py",
    "find . -name '*.py' | xargs node",
    "python -m arbitrary_module",
    "python -m json.tool input.json; node helper.js",
];

#[test]
fn prohibited_commands_return_actionable_denials() -> io::Result<()> {
    for tool in ["Bash", "PowerShell"] {
        for command in PROHIBITED_COMMANDS {
            let output = invoke(&event(tool, command))?;
            assert!(output.status.success(), "{command}: {output:?}");
            let response: Value = serde_json::from_slice(&output.stdout)?;
            let hook = response.get("hookSpecificOutput").unwrap();
            assert_eq!(hook.get("permissionDecision").unwrap(), "deny", "{command}");
            assert_eq!(hook.get("hookEventName").unwrap(), "PreToolUse");
            let reason = hook
                .get("permissionDecisionReason")
                .unwrap()
                .as_str()
                .unwrap();
            assert!(reason.contains("cargo tool --help"), "{command}");
            assert!(reason.contains("smallest tested Rust command"));
            assert!(output.stderr.is_empty());
        }
    }
    Ok(())
}

#[test]
fn ordinary_commands_preserve_normal_permissions() -> io::Result<()> {
    for tool in ["Bash", "PowerShell"] {
        for command in [
            "cargo fmt --all --check",
            "cargo clippy --all-targets --all-features -- -D warnings",
            "cargo test --all-targets && cargo build",
            "cargo tool text replace-text --help",
            "rg python src",
            "rg 'python -c|node -e|for file in files' docs",
            "rg '`|$(python helper.py)' docs",
            "Get-Content -Raw tools/README.md",
            "cargo metadata --no-deps --format-version 1 | jq '.packages[].name'",
            "Get-ChildItem tools | Select-Object Name",
            "git status; git diff --stat",
            "python --version",
            "node --help",
            "python -m json.tool input.json",
            "Get-Content -Raw input.json | py -m json.tool",
            "& 'C:\\Program Files\\Python\\python.exe' -m json.tool input.json",
        ] {
            let output = invoke(&event(tool, command))?;
            assert!(output.status.success(), "{command}: {output:?}");
            assert!(output.stdout.is_empty(), "{command}: {output:?}");
            assert!(output.stderr.is_empty(), "{command}: {output:?}");
        }
    }
    Ok(())
}

#[test]
fn malformed_events_fail_without_an_allow_decision() -> io::Result<()> {
    for input in [
        "",
        "{",
        "null",
        "{}",
        r#"{"hook_event_name":"PostToolUse","tool_name":"Bash","tool_input":{"command":"cargo test"}}"#,
        r#"{"hook_event_name":"PreToolUse","tool_name":"Read"}"#,
        r#"{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{}}"#,
        r#"{"hook_event_name":"PreToolUse","tool_name":"Bash","tool_input":{"command":42}}"#,
        &event("PowerShell", " "),
    ] {
        let output = invoke(input)?;
        assert!(!output.status.success(), "{input}");
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
    Ok(())
}

#[test]
fn configured_hook_covers_both_shells_and_blocks_failures() -> io::Result<()> {
    let settings: Value = serde_json::from_str(include_str!("../../../.claude/settings.json"))?;
    let hook = settings
        .pointer("/hooks/PreToolUse/0")
        .ok_or_else(|| io::Error::other("missing PreToolUse hook"))?;
    assert_eq!(hook.get("matcher").unwrap(), "Bash|PowerShell");
    let handler = hook.pointer("/hooks/0").unwrap();
    assert_eq!(handler.get("type").unwrap(), "command");
    assert_eq!(handler.get("command").unwrap(), "cargo");
    assert_eq!(handler.get("onFailure").unwrap(), "block");
    assert!(handler.get("timeout").unwrap().as_u64().unwrap() > 0);
    let args = handler.get("args").unwrap().as_array().unwrap();
    assert!(args.contains(&json!("${CLAUDE_PROJECT_DIR}/tools/Cargo.toml")));
    assert_eq!(args.last().unwrap(), "agent-command");
    assert!(args.contains(&json!("--quiet")));
    Ok(())
}
