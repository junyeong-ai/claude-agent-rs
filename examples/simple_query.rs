//! Simple query example - make a one-shot request to Claude.
//!
//! Run with: cargo run --example simple_query
//!
//! Requires ANTHROPIC_API_KEY environment variable to be set.

use claude_agent::query;

#[tokio::main]
async fn main() -> Result<(), claude_agent::Error> {
    // Simple one-shot query
    let response = query("What is 2 + 2? Reply with just the number.").await?;
    println!("Response: {}", response);

    Ok(())
}
