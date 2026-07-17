use crate::{Error, Result};

pub fn format_slash_command(command: &str, args: &[impl AsRef<str>]) -> Result<String> {
    let command = command.trim();
    if !command.starts_with('/') || command.chars().any(char::is_whitespace) {
        return Err(Error::InvalidInput(format!(
            "invalid slash command {command:?}"
        )));
    }
    let args = args
        .iter()
        .map(|v| v.as_ref().trim())
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>();
    Ok(if args.is_empty() {
        command.into()
    } else {
        format!("{command} {}", args.join(" "))
    })
}
