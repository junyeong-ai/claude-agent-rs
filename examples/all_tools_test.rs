//! All Built-in Tools Test
//!
//! Run: cargo run --example all_tools_test

use claude_agent::tools::{
    BashTool, EditTool, GlobTool, GrepTool, KillShellTool, ReadTool, Tool, ToolResult,
    TodoWriteTool, WriteTool,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║          모든 빌트인 도구 직접 테스트                          ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    let temp_dir = tempfile::tempdir()?;
    let working_dir = temp_dir.path().to_path_buf();

    let mut passed = 0;
    let mut failed = 0;

    // =========================================================================
    // 1. Bash Tool
    // =========================================================================
    print!("1. BashTool... ");
    let bash = BashTool::new(working_dir.clone());
    let result = bash
        .execute(serde_json::json!({
            "command": "echo 'Hello from Bash'"
        }))
        .await;
    match result {
        ToolResult::Success(output) if output.contains("Hello from Bash") => {
            println!("✓ PASS");
            passed += 1;
        }
        _ => {
            println!("✗ FAIL: {:?}", result);
            failed += 1;
        }
    }

    // =========================================================================
    // 2. Write Tool
    // =========================================================================
    print!("2. WriteTool... ");
    let write = WriteTool::new(working_dir.clone());
    let test_file = working_dir.join("test.txt");
    let result = write
        .execute(serde_json::json!({
            "file_path": test_file.to_str().unwrap(),
            "content": "Hello World from WriteTool"
        }))
        .await;
    match result {
        ToolResult::Success(_) => {
            let content = tokio::fs::read_to_string(&test_file).await?;
            if content.contains("Hello World") {
                println!("✓ PASS");
                passed += 1;
            } else {
                println!("✗ FAIL: content mismatch");
                failed += 1;
            }
        }
        _ => {
            println!("✗ FAIL: {:?}", result);
            failed += 1;
        }
    }

    // =========================================================================
    // 3. Read Tool
    // =========================================================================
    print!("3. ReadTool... ");
    let read = ReadTool::new(working_dir.clone());
    let result = read
        .execute(serde_json::json!({
            "file_path": test_file.to_str().unwrap()
        }))
        .await;
    match result {
        ToolResult::Success(output) if output.contains("Hello World") => {
            println!("✓ PASS");
            passed += 1;
        }
        _ => {
            println!("✗ FAIL: {:?}", result);
            failed += 1;
        }
    }

    // =========================================================================
    // 4. Edit Tool
    // =========================================================================
    print!("4. EditTool... ");
    let edit = EditTool::new(working_dir.clone());
    let result = edit
        .execute(serde_json::json!({
            "file_path": test_file.to_str().unwrap(),
            "old_string": "World",
            "new_string": "Rust"
        }))
        .await;
    match result {
        ToolResult::Success(_) => {
            let content = tokio::fs::read_to_string(&test_file).await?;
            if content.contains("Hello Rust") {
                println!("✓ PASS");
                passed += 1;
            } else {
                println!("✗ FAIL: edit not applied");
                failed += 1;
            }
        }
        _ => {
            println!("✗ FAIL: {:?}", result);
            failed += 1;
        }
    }

    // =========================================================================
    // 5. Glob Tool
    // =========================================================================
    print!("5. GlobTool... ");
    // Create more files
    tokio::fs::write(working_dir.join("file1.rs"), "fn main() {}").await?;
    tokio::fs::write(working_dir.join("file2.rs"), "fn test() {}").await?;
    tokio::fs::write(working_dir.join("readme.md"), "# README").await?;

    let glob = GlobTool::new(working_dir.clone());
    let result = glob
        .execute(serde_json::json!({
            "pattern": "*.rs"
        }))
        .await;
    match result {
        ToolResult::Success(output)
            if output.contains("file1.rs") && output.contains("file2.rs") =>
        {
            println!("✓ PASS");
            passed += 1;
        }
        _ => {
            println!("✗ FAIL: {:?}", result);
            failed += 1;
        }
    }

    // =========================================================================
    // 6. Grep Tool
    // =========================================================================
    print!("6. GrepTool... ");
    let grep = GrepTool::new(working_dir.clone());
    let result = grep
        .execute(serde_json::json!({
            "pattern": "fn main",
            "path": working_dir.to_str().unwrap()
        }))
        .await;
    match result {
        ToolResult::Success(output) if output.contains("file1.rs") => {
            println!("✓ PASS");
            passed += 1;
        }
        _ => {
            println!("✗ FAIL: {:?}", result);
            failed += 1;
        }
    }

    // =========================================================================
    // 7. TodoWrite Tool
    // =========================================================================
    print!("7. TodoWriteTool... ");
    let todo = TodoWriteTool::new();
    let result = todo
        .execute(serde_json::json!({
            "todos": [
                {"content": "Task A", "status": "pending", "activeForm": "Working on A"},
                {"content": "Task B", "status": "in_progress", "activeForm": "Working on B"},
                {"content": "Task C", "status": "completed", "activeForm": "Done C"}
            ]
        }))
        .await;
    match result {
        ToolResult::Success(output) if output.contains("Task") || output.contains("Todo") => {
            println!("✓ PASS");
            passed += 1;
        }
        _ => {
            println!("✗ FAIL: {:?}", result);
            failed += 1;
        }
    }

    // =========================================================================
    // 8. KillShell Tool (error case)
    // =========================================================================
    print!("8. KillShellTool (graceful error)... ");
    let kill = KillShellTool::new();
    let result = kill
        .execute(serde_json::json!({
            "shell_id": "nonexistent_12345"
        }))
        .await;
    // Should return an error but not panic
    match result {
        ToolResult::Error(msg) if msg.contains("not found") => {
            println!("✓ PASS (expected error)");
            passed += 1;
        }
        ToolResult::Success(_) => {
            println!("✗ FAIL: should have returned error");
            failed += 1;
        }
        _ => {
            println!("✗ FAIL: unexpected result {:?}", result);
            failed += 1;
        }
    }

    // =========================================================================
    // 9. Bash Background Process
    // =========================================================================
    print!("9. BashTool (background)... ");
    let bash_bg = BashTool::new(working_dir.clone());
    let result = bash_bg
        .execute(serde_json::json!({
            "command": "sleep 0.1 && echo 'background done'",
            "run_in_background": true
        }))
        .await;
    match result {
        ToolResult::Success(output) if output.contains("Background") || output.contains("process") => {
            println!("✓ PASS");
            passed += 1;
        }
        _ => {
            println!("✗ FAIL: {:?}", result);
            failed += 1;
        }
    }

    // =========================================================================
    // 10. Read with offset/limit
    // =========================================================================
    print!("10. ReadTool (offset/limit)... ");
    let multiline_file = working_dir.join("multiline.txt");
    tokio::fs::write(&multiline_file, "line1\nline2\nline3\nline4\nline5").await?;

    let read2 = ReadTool::new(working_dir.clone());
    let result = read2
        .execute(serde_json::json!({
            "file_path": multiline_file.to_str().unwrap(),
            "offset": 1,
            "limit": 2
        }))
        .await;
    match result {
        ToolResult::Success(output) if output.contains("line2") && output.contains("line3") => {
            println!("✓ PASS");
            passed += 1;
        }
        _ => {
            println!("✗ FAIL: {:?}", result);
            failed += 1;
        }
    }

    // =========================================================================
    // Summary
    // =========================================================================
    println!();
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  결과: {} 통과, {} 실패", passed, failed);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    if failed == 0 {
        println!("╔══════════════════════════════════════════════════════════════╗");
        println!("║              모든 도구 테스트 통과! ✓                         ║");
        println!("╚══════════════════════════════════════════════════════════════╝");
    } else {
        println!("╔══════════════════════════════════════════════════════════════╗");
        println!("║              일부 테스트 실패! ✗                               ║");
        println!("╚══════════════════════════════════════════════════════════════╝");
    }

    Ok(())
}
