use std::{
    path::PathBuf,
    process::{Child, Command, Stdio},
};

pub fn has_tracesets() -> bool {
    let mut script_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    script_path.push("../kernel_has_tracesets.sh");
    let process = match Command::new(script_path).stdout(Stdio::piped()).spawn() {
        Ok(process) => process,
        Err(err) => panic!("could not run kernel patch detection script: {}", err),
    };
    let output = match process.wait_with_output() {
        Ok(output) => output,
        Err(why) => panic!("couldn't read script stdout: {}", why),
    };
    let output = String::from_utf8(output.stdout).expect("valid utf8");
    output.starts_with("yes")
}

// need to wrap child process so we can auto cleanup when tests panic
pub struct ProcessWrapper {
    pub process: Child,
}

impl Drop for ProcessWrapper {
    fn drop(&mut self) {
        let _ = self.process.kill();
    }
}

pub fn spawn_echoer() -> ProcessWrapper {
    ProcessWrapper {
        process: Command::new("bash")
            .arg("-c")
            .arg("while true; do echo hi; sleep 1; done")
            .stdout(Stdio::null())
            .spawn()
            .expect("bash command to exist"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time};

    #[test]
    fn test_echoer() {
        let mut sleeper = spawn_echoer();
        thread::sleep(time::Duration::from_millis(50));
        sleeper.process.kill().expect("no process to kill");
    }
}
