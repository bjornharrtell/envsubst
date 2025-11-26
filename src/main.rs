use clap::Parser;
use std::collections::HashSet;
use std::env;
use std::io::{self, Read, Write};

#[derive(Parser)]
#[command(name = "envsubst")]
#[command(about = "Substitutes environment variables in shell format strings", long_about = None)]
struct Cli {
    /// List variables occurring in SHELL-FORMAT
    #[arg(long)]
    variables: bool,

    /// Shell format string specifying which variables to substitute
    /// If provided, only variables in this string will be substituted
    /// If not provided, all variables will be substituted
    shell_format: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    // Read input from stdin
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .expect("Failed to read from stdin");

    // Determine which variables to substitute
    let allowed_vars = if let Some(ref shell_format) = cli.shell_format {
        let vars = extract_variables(shell_format);
        Some(vars.into_iter().collect::<HashSet<String>>())
    } else {
        None
    };

    if cli.variables {
        // List all variables found in the shell format or input
        let vars = if let Some(ref shell_format) = cli.shell_format {
            extract_variables(shell_format)
        } else {
            extract_variables(&input)
        };
        for var in vars {
            println!("{}", var);
        }
    } else {
        // Perform substitution
        let result = substitute_variables(&input, allowed_vars.as_ref());
        print!("{}", result);
        io::stdout().flush().unwrap();
    }
}

/// Parse a variable reference starting after the '$' character
/// Returns (variable_name, is_braced) where is_braced indicates ${VAR} syntax
fn parse_variable(chars: &mut std::iter::Peekable<std::str::Chars>) -> Option<(String, bool)> {
    match chars.peek().copied()? {
        '{' => {
            chars.next(); // consume '{'
            let var_name = consume_until(chars, '}');
            Some((var_name, true))
        }
        ch if is_var_start(ch) => {
            let var_name = consume_var_name(chars);
            if !var_name.is_empty() {
                Some((var_name, false))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Extract all variable names from the input string
fn extract_variables(input: &str) -> Vec<String> {
    let mut vars = HashSet::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' {
            if let Some((var_name, _)) = parse_variable(&mut chars) {
                if !var_name.is_empty() {
                    vars.insert(var_name);
                }
            }
        }
    }

    let mut result: Vec<String> = vars.into_iter().collect();
    result.sort();
    result
}

/// Get the value to substitute for a variable name
/// Returns Some(value) if substitution should happen (value may be empty if var not found)
/// Returns None if the variable should not be substituted (keep original)
fn get_substitution_value(var_name: &str, allowed_vars: Option<&HashSet<String>>) -> Option<String> {
    if should_substitute(var_name, allowed_vars) {
        Some(env::var(var_name).unwrap_or_default())
    } else {
        None
    }
}

/// Reconstruct the original variable syntax
fn reconstruct_variable(var_name: &str, is_braced: bool) -> String {
    if is_braced {
        format!("${{{}}}", var_name)
    } else {
        format!("${}", var_name)
    }
}

/// Substitute environment variables in the input string
fn substitute_variables(input: &str, allowed_vars: Option<&HashSet<String>>) -> String {
    let mut result = String::new();
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' {
            match parse_variable(&mut chars) {
                Some((var_name, is_braced)) => {
                    match get_substitution_value(&var_name, allowed_vars) {
                        Some(value) => result.push_str(&value),
                        None => result.push_str(&reconstruct_variable(&var_name, is_braced)),
                    }
                }
                None => result.push(ch),
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Check if a character can start a variable name (letter or underscore)
fn is_var_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

/// Check if a character can be part of a variable name (letter, digit, or underscore)
fn is_var_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

/// Consume characters until the delimiter is found
fn consume_until(chars: &mut std::iter::Peekable<std::str::Chars>, delimiter: char) -> String {
    let mut result = String::new();
    while let Some(&ch) = chars.peek() {
        if ch == delimiter {
            chars.next(); // consume the delimiter
            break;
        }
        result.push(ch);
        chars.next();
    }
    result
}

/// Consume a variable name (alphanumeric and underscore)
fn consume_var_name(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut result = String::new();
    while let Some(&ch) = chars.peek() {
        if !is_var_char(ch) {
            break;
        }
        result.push(ch);
        chars.next();
    }
    result
}

/// Check if a variable should be substituted based on the allowed list
fn should_substitute(var_name: &str, allowed_vars: Option<&HashSet<String>>) -> bool {
    match allowed_vars {
        Some(set) => set.contains(var_name),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_variables_simple() {
        let input = "Hello $USER, your home is $HOME";
        let vars = extract_variables(input);
        assert_eq!(vars, vec!["HOME", "USER"]);
    }

    #[test]
    fn test_extract_variables_braced() {
        let input = "Path: ${PATH}, Shell: ${SHELL}";
        let vars = extract_variables(input);
        assert_eq!(vars, vec!["PATH", "SHELL"]);
    }

    #[test]
    fn test_extract_variables_mixed() {
        let input = "$USER lives in ${HOME} and uses $SHELL";
        let vars = extract_variables(input);
        assert_eq!(vars, vec!["HOME", "SHELL", "USER"]);
    }

    #[test]
    fn test_extract_variables_duplicates() {
        let input = "$USER and $USER again";
        let vars = extract_variables(input);
        assert_eq!(vars, vec!["USER"]);
    }

    #[test]
    fn test_extract_variables_empty() {
        let input = "No variables here";
        let vars = extract_variables(input);
        assert!(vars.is_empty());
    }

    #[test]
    fn test_extract_variables_invalid() {
        let input = "$ $123 ${} $";
        let vars = extract_variables(input);
        assert!(vars.is_empty());
    }

    #[test]
    fn test_substitute_variables_simple() {
        unsafe {
            env::set_var("TEST_VAR", "test_value");
        }
        let input = "Value: $TEST_VAR";
        let result = substitute_variables(input, None);
        assert_eq!(result, "Value: test_value");
        unsafe {
            env::remove_var("TEST_VAR");
        }
    }

    #[test]
    fn test_substitute_variables_braced() {
        unsafe {
            env::set_var("TEST_VAR", "braced_value");
        }
        let input = "Value: ${TEST_VAR}";
        let result = substitute_variables(input, None);
        assert_eq!(result, "Value: braced_value");
        unsafe {
            env::remove_var("TEST_VAR");
        }
    }

    #[test]
    fn test_substitute_variables_undefined() {
        unsafe {
            env::remove_var("UNDEFINED_VAR_12345");
        }
        let input = "Value: $UNDEFINED_VAR_12345";
        let result = substitute_variables(input, None);
        assert_eq!(result, "Value: ");
    }

    #[test]
    fn test_substitute_variables_mixed() {
        unsafe {
            env::set_var("VAR1", "value1");
            env::set_var("VAR2", "value2");
        }
        let input = "$VAR1 and ${VAR2}";
        let result = substitute_variables(input, None);
        assert_eq!(result, "value1 and value2");
        unsafe {
            env::remove_var("VAR1");
            env::remove_var("VAR2");
        }
    }

    #[test]
    fn test_substitute_variables_with_filter() {
        unsafe {
            env::set_var("VAR1", "value1");
            env::set_var("VAR2", "value2");
            env::set_var("VAR3", "value3");
        }
        
        let mut allowed = HashSet::new();
        allowed.insert("VAR1".to_string());
        allowed.insert("VAR3".to_string());
        
        let input = "$VAR1 $VAR2 $VAR3";
        let result = substitute_variables(input, Some(&allowed));
        assert_eq!(result, "value1 $VAR2 value3");
        
        unsafe {
            env::remove_var("VAR1");
            env::remove_var("VAR2");
            env::remove_var("VAR3");
        }
    }

    #[test]
    fn test_substitute_variables_adjacent() {
        unsafe {
            env::set_var("A", "foo");
            env::set_var("B", "bar");
        }
        let input = "$A$B";
        let result = substitute_variables(input, None);
        assert_eq!(result, "foobar");
        unsafe {
            env::remove_var("A");
            env::remove_var("B");
        }
    }

    #[test]
    fn test_substitute_variables_in_text() {
        unsafe {
            env::set_var("NAME", "World");
        }
        let input = "Hello, $NAME!";
        let result = substitute_variables(input, None);
        assert_eq!(result, "Hello, World!");
        unsafe {
            env::remove_var("NAME");
        }
    }

    #[test]
    fn test_substitute_lone_dollar() {
        let input = "Price: $100";
        let result = substitute_variables(input, None);
        assert_eq!(result, "Price: $100");
    }

    #[test]
    fn test_substitute_dollar_at_end() {
        let input = "ends with $";
        let result = substitute_variables(input, None);
        assert_eq!(result, "ends with $");
    }

    #[test]
    fn test_is_var_start() {
        assert!(is_var_start('a'));
        assert!(is_var_start('Z'));
        assert!(is_var_start('_'));
        assert!(!is_var_start('1'));
        assert!(!is_var_start('-'));
        assert!(!is_var_start('$'));
    }

    #[test]
    fn test_is_var_char() {
        assert!(is_var_char('a'));
        assert!(is_var_char('Z'));
        assert!(is_var_char('_'));
        assert!(is_var_char('0'));
        assert!(is_var_char('9'));
        assert!(!is_var_char('-'));
        assert!(!is_var_char('$'));
        assert!(!is_var_char(' '));
    }

    #[test]
    fn test_empty_braces() {
        let input = "${}";
        let result = substitute_variables(input, None);
        assert_eq!(result, "");
    }

    #[test]
    fn test_unclosed_braces() {
        unsafe {
            env::set_var("VAR", "value");
        }
        let input = "${VAR";
        let result = substitute_variables(input, None);
        // Unclosed brace consumes rest of string as variable name
        assert_eq!(result, "value");
        unsafe {
            env::remove_var("VAR");
        }
    }

    #[test]
    fn test_variable_with_underscores_and_numbers() {
        unsafe {
            env::set_var("MY_VAR_123", "test");
        }
        let input = "$MY_VAR_123";
        let result = substitute_variables(input, None);
        assert_eq!(result, "test");
        unsafe {
            env::remove_var("MY_VAR_123");
        }
    }

    #[test]
    fn test_variable_stops_at_special_char() {
        unsafe {
            env::set_var("VAR", "value");
        }
        let input = "$VAR-suffix";
        let result = substitute_variables(input, None);
        assert_eq!(result, "value-suffix");
        unsafe {
            env::remove_var("VAR");
        }
    }
}
