use std::fmt::Display;

pub fn user_error(context: &str, err: impl Display) -> String {
    format!("sow-tools: {context}: {err}")
}

pub fn exit_user_error(context: &str, err: impl Display) -> ! {
    eprintln!("{}", user_error(context, err));
    std::process::exit(1);
}
