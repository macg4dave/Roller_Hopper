//! Conservative command checks for Claude's shell tools, never a shell executor.

use std::io::{self, Read, Write};

use regex::Regex;
use serde_json::{Value, json};

const GUIDANCE: &str = "Scripts must be saved in the tools/ folder and run from there: inline code and scripts saved anywhere else are blocked. Read tools/README.md and run cargo tool --help first; use existing tooling when it fits, otherwise save the script under tools/ (see tools/README.md, Scripts) and run it by that path.";

pub(super) fn run() -> io::Result<()> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    if let Some(reason) = decision(&input)? {
        writeln!(
            io::stdout().lock(),
            "{}",
            json!({"hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny",
                "permissionDecisionReason": format!("{reason} {GUIDANCE}")
            }})
        )?;
    }
    // No allow decision: preserve the normal permission flow.
    Ok(())
}

fn decision(input: &str) -> io::Result<Option<&'static str>> {
    let event: Value = serde_json::from_str(input).map_err(io::Error::other)?;
    if event.get("hook_event_name").and_then(Value::as_str) != Some("PreToolUse") {
        return Err(io::Error::other("expected a PreToolUse event"));
    }
    match event.get("tool_name").and_then(Value::as_str) {
        Some("Bash" | "PowerShell") => {}
        _ => return Err(io::Error::other("expected Bash or PowerShell tool input")),
    }
    let command = event
        .get("tool_input")
        .and_then(|value| value.get("command"))
        .and_then(Value::as_str)
        .filter(|command| !command.trim().is_empty())
        .ok_or_else(|| io::Error::other("missing non-empty tool_input.command"))?;
    check(command)
}

fn check(command: &str) -> io::Result<Option<&'static str>> {
    // Recognise quoted arguments and punctuation, rather than splitting quoted data on pipes.
    // This is intentionally a bounded detector, not a complete Bash/PowerShell grammar.
    let lexer = Regex::new(r#"'[^']*'|"[^"\r\n]*"|[;&|(){}<>\r\n]|[^\s'";&|(){}<>]+"#)
        .map_err(io::Error::other)?;
    let tokens: Vec<_> = lexer.find_iter(command).map(|item| item.as_str()).collect();
    let mut executable = true;
    for (index, token) in tokens.iter().enumerate() {
        if matches!(*token, "{" | "}" | "<")
            || (!token.starts_with('\'')
                && (token.contains('`') || token.contains("$(") || *token == "$"))
        {
            return Ok(Some(
                "Shell programs, substitutions and input redirection are blocked.",
            ));
        }
        if matches!(*token, ";" | "&" | "|" | "(" | ")" | "\r" | "\n") {
            executable = true;
            continue;
        }
        if !executable {
            continue;
        }
        let path = token
            .trim_matches(['\'', '"'])
            .replace('\\', "/")
            .to_ascii_lowercase();
        if path.starts_with(['$', '@'])
            || path.contains("/tmp/")
            || path.contains("/temp/")
            || path.contains("%temp%")
        {
            return Ok(Some(
                "Dynamic commands and temporary executable paths are blocked.",
            ));
        }
        let name = executable_name(token);
        if matches!(name.as_str(), "env" | "command" | "exec" | "sudo" | "nohup")
            || token.contains('=')
        {
            continue;
        }
        if saved_tool_script(token) {
            executable = false;
            continue;
        }
        if let Some(reason) = blocked_executable(&name, tokens.get(index + 1..).unwrap_or_default())
        {
            return Ok(Some(reason));
        }
        executable = false;
    }
    Ok(None)
}

fn executable_name(token: &str) -> String {
    token
        .trim_matches(['\'', '"'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .trim_end_matches(".exe")
        .to_owned()
}

fn blocked_executable(name: &str, tail: &[&str]) -> Option<&'static str> {
    let arguments: Vec<_> = tail
        .iter()
        .take_while(|token| !matches!(**token, ";" | "&" | "|" | "\r" | "\n"))
        .map(|token| token.trim_matches(['\'', '"']))
        .collect();
    if is_interpreter(name)
        && !maintained_entry_point(name, &arguments)
        && !runs_saved_script(&arguments)
    {
        return Some("Unapproved interpreter or script invocation.");
    }
    if matches!(
        name,
        "for"
            | "foreach"
            | "while"
            | "until"
            | "do"
            | "function"
            | "filter"
            | "if"
            | "case"
            | "select"
            | "switch"
            | "try"
            | "foreach-object"
            | "%"
            | "invoke-expression"
            | "iex"
            | "eval"
            | "source"
            | "."
            | "invoke-command"
            | "start-process"
            | "start-job"
    ) || [
        ".py", ".js", ".mjs", ".cjs", ".ts", ".ps1", ".sh", ".bash", ".cmd", ".bat", ".rs", ".pl",
        ".rb", ".lua",
    ]
    .iter()
    .any(|extension| name.ends_with(extension))
    {
        return Some("Custom shell programs and unapproved script entry points are blocked.");
    }
    if matches!(name, "xargs" | "parallel")
        && arguments
            .iter()
            .any(|argument| blocked_executable(&executable_name(argument), &[]).is_some())
    {
        return Some("A command wrapper invokes an unapproved script or interpreter.");
    }
    None
}

fn is_interpreter(name: &str) -> bool {
    name.starts_with("python")
        || matches!(
            name,
            "py" | "node"
                | "nodejs"
                | "ruby"
                | "perl"
                | "lua"
                | "luajit"
                | "deno"
                | "bun"
                | "ts-node"
                | "tsx"
                | "powershell"
                | "pwsh"
                | "bash"
                | "sh"
                | "zsh"
                | "dash"
                | "cmd"
                | "cscript"
                | "wscript"
                | "rust-script"
        )
}

/// An interpreter given a script saved under `tools/`, after only options that do not supply
/// code themselves (`python -I tools/x.py`, `powershell -File tools/x.ps1`, `cmd /c tools\x.cmd`).
fn runs_saved_script(arguments: &[&str]) -> bool {
    let Some(script) = arguments.iter().position(|a| saved_tool_script(a)) else {
        return false;
    };
    let mut before = arguments.iter().take(script);
    let mut value_of_option = false;
    before.all(|argument| {
        let option = argument.to_ascii_lowercase();
        if std::mem::take(&mut value_of_option) {
            return !option.starts_with('-');
        }
        value_of_option = option == "-executionpolicy";
        (option.starts_with('-') || option == "/c")
            && !matches!(
                option.as_str(),
                "-" | "-c"
                    | "-e"
                    | "-p"
                    | "-m"
                    | "--eval"
                    | "--print"
                    | "-command"
                    | "-encodedcommand"
                    | "-ec"
            )
    })
}

/// A path to a script saved in the repository's `tools/` folder, relative to the repository
/// root and never climbing out of it.
fn saved_tool_script(token: &str) -> bool {
    let path = token
        .trim_matches(['\'', '"'])
        .replace('\\', "/")
        .to_ascii_lowercase();
    let path = path.strip_prefix("./").unwrap_or(&path);
    path.strip_prefix("tools/")
        .is_some_and(|rest| rest.split('/').all(|part| !matches!(part, "" | "." | "..")))
}

fn maintained_entry_point(name: &str, arguments: &[&str]) -> bool {
    // Explicit maintained entry points only; a repository path is never blanket approval.
    matches!(arguments, ["--version" | "--help"])
        || ((name.starts_with("python") || name == "py")
            && arguments.starts_with(&["-m", "json.tool"]))
}
