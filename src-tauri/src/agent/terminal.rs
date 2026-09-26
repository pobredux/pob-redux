//! Opening a visible terminal that runs a vendor's own install or sign-in
//! command, so the user sees it and can answer whatever it asks.

use std::ffi::OsString;

/// Quote for PowerShell: single quotes, with embedded ones doubled.
pub(crate) fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Quote for a POSIX shell.
pub(crate) fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

#[cfg(windows)]
pub(crate) fn open(title: &str, powershell: &str, _sh: &str, path: &OsString) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    // `start` gives the command its own console; the inner command keeps only single quotes so cmd leaves it alone.
    let line = format!(
        "/c start \"{}\" powershell.exe -NoExit -NoProfile -ExecutionPolicy Bypass -Command \"{}\"",
        title.replace('"', ""),
        powershell.replace('"', "'")
    );
    std::process::Command::new("cmd.exe")
        .raw_arg(line)
        .env("PATH", path)
        .creation_flags(0x0800_0000)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("could not open a terminal: {e}"))
}

#[cfg(unix)]
fn reap(mut child: std::process::Child) {
    std::thread::spawn(move || {
        let _ = child.wait();
    });
}

/// `open` on a `.command` file needs no Apple Events permission, which scripting Terminal would.
#[cfg(target_os = "macos")]
pub(crate) fn open(title: &str, _ps: &str, sh: &str, path: &OsString) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let name: String = title.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' { c } else { '-' }).collect();
    let file = std::env::temp_dir().join(format!("{name}.command"));
    let script = format!(
        "#!/bin/sh\nrm -f \"$0\"\nexport PATH={}\n{sh}\necho\necho 'Done. You can close this window.'\n",
        sh_quote(&path.to_string_lossy())
    );
    std::fs::write(&file, script).map_err(|e| format!("{}: {e}", file.display()))?;
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o700)).map_err(|e| format!("{}: {e}", file.display()))?;
    let child = std::process::Command::new("/usr/bin/open").arg(&file).spawn().map_err(|e| format!("could not open Terminal: {e}"))?;
    reap(child);
    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) fn open(_title: &str, _ps: &str, sh: &str, path: &OsString) -> Result<(), String> {
    let script = format!("{sh}; echo; echo 'Done. You can close this window.'; exec \"${{SHELL:-sh}}\"");
    let tries: [(&str, &[&str]); 12] = [
        ("x-terminal-emulator", &["-e", "sh", "-c"]),
        ("xdg-terminal-exec", &["sh", "-c"]),
        ("gnome-terminal", &["--", "sh", "-c"]),
        ("ptyxis", &["--", "sh", "-c"]),
        ("kgx", &["--", "sh", "-c"]),
        ("konsole", &["-e", "sh", "-c"]),
        ("xfce4-terminal", &["-x", "sh", "-c"]),
        ("kitty", &["sh", "-c"]),
        ("alacritty", &["-e", "sh", "-c"]),
        ("wezterm", &["start", "--", "sh", "-c"]),
        ("foot", &["sh", "-c"]),
        ("xterm", &["-e", "sh", "-c"]),
    ];
    for (term, args) in tries {
        let mut cmd = std::process::Command::new(term);
        super::which::restore_host_env(&mut cmd);
        if let Ok(child) = cmd.args(args).arg(&script).env("PATH", path).spawn() {
            reap(child);
            return Ok(());
        }
    }
    Err(format!("No terminal program was found. Run this yourself: {sh}"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn quoting_survives_embedded_quotes() {
        assert_eq!(super::ps_quote("C:\\it's\\claude.exe"), "'C:\\it''s\\claude.exe'");
        assert_eq!(super::sh_quote("/opt/it's/claude"), "'/opt/it'\\''s/claude'");
    }
}
