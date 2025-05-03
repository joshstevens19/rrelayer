use colored::Colorize;
use prettytable::{Cell, Row, Table, format};

pub fn print_error_message(error_message: &str) {
    println!("{}", error_message.red());
}

pub fn print_warn_message(error_message: &str) {
    println!("{}", error_message.yellow());
}

pub fn print_success_message(success_message: &str) {
    println!("{}", success_message.green());
}

pub fn print_table<T: AsRef<str>>(
    headers: Vec<T>,
    rows: Vec<Vec<String>>,
    title: Option<&str>,
    footer: Option<&str>,
) {
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_NO_BORDER_LINE_SEPARATOR);

    let header_cells =
        headers.into_iter().map(|h| Cell::new(h.as_ref()).style_spec("Fb")).collect();
    table.add_row(Row::new(header_cells));

    for row_data in rows {
        let cells = row_data.into_iter().map(|cell| Cell::new(cell.as_ref())).collect();
        table.add_row(Row::new(cells));
    }

    if let Some(title_text) = title {
        println!("\n{}", title_text);
    }

    table.printstd();

    if let Some(footer_text) = footer {
        println!("\n{}", footer_text);
    }
}
