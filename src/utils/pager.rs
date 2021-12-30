use anyhow::{format_err, Result};
use std::{
    process::{Child, ChildStdin},
    sync::atomic::Ordering,
};

pub struct Pager {
    pager_name: String,
    child: Child,
}

impl Pager {
    pub fn new() -> Result<Self> {
        let pager_cmd = std::env::var("PAGER").unwrap_or_else(|_| "less".to_owned());
        let pager_cmd_segments: Vec<&str> = pager_cmd.split_ascii_whitespace().collect();
        let pager_name = pager_cmd_segments.get(0).unwrap_or(&"less");
        let mut p = std::process::Command::new(&pager_name);
        if pager_name == &"less" {
            p.arg("-R"); // Show ANSI escape sequences correctly
            p.arg("-c"); // Start from the top of the screen
            p.env("LESSCHARSET", "UTF-8"); // Rust uses UTF-8
        } else if pager_cmd_segments.len() > 1 {
            p.args(&pager_cmd_segments[1..]);
        }
        let pager_process = p.stdin(std::process::Stdio::piped()).spawn()?;
        // Record PID
        crate::SUBPROCESS.store(pager_process.id() as i32, Ordering::SeqCst);

        let res = Pager {
            pager_name: pager_name.to_string(),
            child: pager_process,
        };
        Ok(res)
    }

    pub fn pager_name(&self) -> &str {
        &self.pager_name
    }

    pub fn get_writer<'a>(&'a mut self) -> Result<&'a mut ChildStdin> {
        let stdin = self
            .child
            .stdin
            .as_mut()
            .ok_or_else(|| format_err!("Failed to take pager's stdin"))?;
        Ok(stdin)
    }

    pub fn wait_for_exit(&mut self) -> Result<()> {
        let _ = self.child.wait()?;
        Ok(())
    }
}

impl Drop for Pager {
    fn drop(&mut self) {
        // Un-set subprocess pid
        crate::SUBPROCESS.store(-1, Ordering::SeqCst);
    }
}
