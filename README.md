# Data Lineage Tracker

A tool for analyzing variable declarations and their usage across JavaScript code using tree-sitter.

## Installation

1. Make sure you have Rust and Cargo installed. If not, install from [rustup.rs](https://rustup.rs/):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rust-lang.org | sh
```

2. Clone the repository:
```bash
git clone https://github.com/yourusername/data_lineage_tracker.git
cd data_lineage_tracker
```

3. Build the project:
```bash
cargo build
```

## Usage

### Running the Program

You can run the program in several ways:

1. Using cargo run:
```bash
cargo run -- path/to/your/javascript/file.js
```

2. Or build and run the binary directly:
```bash
cargo build
./target/debug/data_lineage_tracker path/to/your/javascript/file.js
```

### Example

1. Create a test JavaScript file:
```bash
echo '
const globalVar = 42;
function outer() {
    let outerVar = globalVar + 1;
    function inner() {
        const innerVar = outerVar * 2;
        return innerVar;
    }
    return inner() + outerVar;
}
class Example {
    constructor() {
        this.classVar = globalVar;
    }
    method() {
        return this.classVar + globalVar;
    }
}
' > test.js
```

2. Run the analyzer:
```bash
cargo run -- test.js
```

## Testing

Run the test suite:
```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test
cargo test test_variable_tracking

# Run tests with debug information
RUST_BACKTRACE=1 cargo test
```
