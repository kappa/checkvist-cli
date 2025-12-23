// Standalone fake Checkvist API server for manual testing
// Run with: cargo run --bin fake-server

#[path = "fake_server/mod.rs"]
mod fake_server;

fn main() {
    let server = fake_server::FakeServer::new();

    println!("\n===========================================");
    println!("Fake Checkvist API Server");
    println!("===========================================");
    println!();
    println!("Server running at: {}", server.base_url());
    println!();
    println!("Test commands:");
    println!();
    println!("# Create list with WRONG parameter (should return 'Name this list'):");
    println!("curl -X POST {}/checklists.json \\", server.base_url());
    println!("  -H 'X-Client-Token: TEST' \\");
    println!("  --data-urlencode 'name=My List'");
    println!();
    println!("# Create list with CORRECT parameter (should use provided name):");
    println!("curl -X POST {}/checklists.json \\", server.base_url());
    println!("  -H 'X-Client-Token: TEST' \\");
    println!("  --data-urlencode 'checklist[name]=My List'");
    println!();
    println!("# List all checklists:");
    println!("curl -H 'X-Client-Token: TEST' {}/checklists.json", server.base_url());
    println!();
    println!("# Test without auth (should return 401):");
    println!("curl {}/checklists.json", server.base_url());
    println!();
    println!("===========================================");
    println!("Press Ctrl+C to stop");
    println!("===========================================");
    println!();

    // Keep server alive
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}
