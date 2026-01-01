//! Agent loop example - run an agent that can use tools.
//!
//! Run with: cargo run --example agent_loop
//!
//! Requires ANTHROPIC_API_KEY environment variable to be set.

use claude_agent::{Agent, AgentEvent, ToolAccess};
use futures::StreamExt;
use std::pin::pin;

#[tokio::main]
async fn main() -> Result<(), claude_agent::Error> {
    // Create an agent with tool access
    let agent = Agent::builder()
        .model("claude-sonnet-4-5-20250514")
        .tools(ToolAccess::all())
        .working_dir(".")
        .build().await?;

    println!("Running agent...\n");

    // Execute with streaming
    let stream = agent
        .execute_stream("List the files in the current directory using the Glob tool")
        .await?;

    // Pin the stream for iteration
    let mut stream = pin!(stream);

    while let Some(event) = stream.next().await {
        match event? {
            AgentEvent::Text(text) => print!("{}", text),
            AgentEvent::ToolStart { name, .. } => {
                println!("\n[Tool: {}]", name);
            }
            AgentEvent::ToolEnd {
                output, is_error, ..
            } => {
                if is_error {
                    println!("[Error: {}]", output);
                } else {
                    println!("[Result: {} bytes]", output.len());
                }
            }
            AgentEvent::Complete(result) => {
                println!(
                    "\n\nComplete! {} tokens, {} tool calls",
                    result.total_tokens(),
                    result.tool_calls
                );
            }
            _ => {}
        }
    }

    Ok(())
}
