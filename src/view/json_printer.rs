use serde_json::Value as JsonValue;

/// Helper function to serialize a JsonValue to a string (pretty or compact)
/// and print it to stdout, or print an error to stderr.
pub fn print_json_output(json_value: &JsonValue, pretty: bool) {
    let result = if pretty {
        serde_json::to_string_pretty(json_value)
    } else {
        serde_json::to_string(json_value)
    };

    match result {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("Error serializing to JSON: {e}");
        }
    }
}
