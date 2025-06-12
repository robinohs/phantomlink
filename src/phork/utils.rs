use std::process::{Command, ExitStatus, Output};

// pub(crate) fn log(level: &str, args: &[String]) -> Result<()> {
//     let namespace = netns_out(&["identify"])?;
//     let username = get_current_username().expect("Should work on this system.");
//     println!(
//         "{}\tns: \"{:12}\"\tu: {:?} a: {:?}",
//         level,
//         namespace,
//         username,
//         &args[1..]
//     );
//     Ok(())
// }

#[track_caller]
pub(crate) fn exec(cmd: &str, args: &[&str]) -> eyre::Result<ExitStatus> {
    let res = Command::new(cmd).args(args).status()?;
    if res.success() {
        Ok(res)
    } else {
        Err(eyre::eyre!("Fail: `{} {}`", cmd, args.join(" ")))
    }
}

// #[track_caller]
// pub(crate) fn spawn(cmd: &str, args: &[&str]) -> Result<Child> {
//     Ok(Command::new(cmd).args(args).spawn()?)
// }

#[track_caller]
pub(crate) fn ip(cmd: &str, args: &[&str]) -> eyre::Result<Output> {
    let res = Command::new("ip").arg(cmd).args(args).output()?;
    if res.status.success() {
        Ok(res)
    } else {
        Err(eyre::eyre!(
            "Fail: `ip {} {}`\n{}\n{}",
            cmd,
            args.join(" "),
            String::from_utf8(res.stdout)?,
            String::from_utf8(res.stderr)?
        ))
    }
}

#[track_caller]
pub(crate) fn netns(args: &[&str]) -> eyre::Result<Output> {
    ip("netns", args)
}

#[track_caller]
pub(crate) fn link(args: &[&str]) -> eyre::Result<Output> {
    ip("link", args)
}

// #[track_caller]
// pub(crate) fn netns_out(args: &[&str]) -> Result<String> {
//     let out = Command::new("ip").arg("netns").args(args).output()?.stdout;
//     Ok(String::from_utf8(out)?.trim().to_string())
// }
