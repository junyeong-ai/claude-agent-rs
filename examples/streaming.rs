//! Streaming example - stream a response from Claude.
//!
//! Run with: cargo run --example streaming
//!
//! Requires ANTHROPIC_API_KEY environment variable to be set.

use claude_agent::stream;
use futures::StreamExt;
use std::pin::pin;

#[tokio::main]
async fn main() -> Result<(), claude_agent::Error> {
    println!("Streaming response:");

    let stream = stream("Tell me a short story about a robot (2-3 sentences)").await?;
    let mut stream = pin!(stream);

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(text) => print!("{}", text),
            Err(e) => eprintln!("\nError: {}", e),
        }
    }

    println!("\n\nDone!");
    Ok(())
}
