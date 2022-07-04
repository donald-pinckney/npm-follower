use semver_spec_serialization::parse_spec_via_node;

// TODO: delete when done
pub fn main() {
    // repl for debugging
    loop {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();
        if input == "quit" {
            break;
        }
        let spec = parse_spec_via_node(input);
        println!("{:?}", spec);
    }
}