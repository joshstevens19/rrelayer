use colored::Colorize;

pub fn print_error_message(error_message: &str) {
    println!("{}", error_message.red());
}

pub fn print_warn_message(error_message: &str) {
    println!("{}", error_message.yellow());
}

pub fn print_success_message(success_message: &str) {
    println!("{}", success_message.green());
}
