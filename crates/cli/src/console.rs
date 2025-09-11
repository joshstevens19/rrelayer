use colored::Colorize;
use prettytable::{Cell, Row, Table, format};

/// Prints an error message to the console in red color.
///
/// Uses colored output to highlight error messages for better visibility.
///
/// # Arguments
/// * `error_message` - The error message to display
pub fn print_error_message(error_message: &str) {
    println!("{}", error_message.red());
}

/// Prints a success message to the console in green color.
///
/// # Arguments
/// * `success_message` - The success message to display
pub fn print_success_message(success_message: &str) {
    println!("{}", success_message.green());
}

/// Prints a formatted table to the console with optional title and footer.
///
/// Creates a nicely formatted table using prettytable with bold headers
/// and optional title/footer text.
///
/// # Arguments
/// * `headers` - Column headers for the table
/// * `rows` - Data rows, each containing a vector of cell values
/// * `title` - Optional title text to display above the table
/// * `footer` - Optional footer text to display below the table
///
/// # Type Parameters
/// * `T` - Type that can be converted to string reference (AsRef<str>)
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
