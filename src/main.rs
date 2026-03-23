use std::io::{self, BufRead, Write};
use std::net::TcpStream;
use std::process::Command;
use std::thread;
use std::time::Duration;

const TUNNEL_PORT: u16 = 1455;
const TUNNEL_BIND: &str = "127.0.0.1";
const TUNNEL_TIMEOUT_SECS: u64 = 15;

fn main() {
    println!("=== Codex SSH Tunnel Helper ===\n");

    let vps_host = get_vps_host();
    let forward_arg = format!(
        "{}:{}:{}:{}",
        TUNNEL_BIND, TUNNEL_PORT, TUNNEL_BIND, TUNNEL_PORT
    );
    let remote = format!("root@{}", vps_host);

    println!(
        "[*] Spawning tunnel: localhost:{} -> {}:{}",
        TUNNEL_PORT, vps_host, TUNNEL_PORT
    );

    let mut ssh = Command::new("ssh")
        .args([
            "-N",
            "-o",
            "StrictHostKeyChecking=accept-new",
            "-o",
            "ExitOnForwardFailure=yes",
            "-L",
            &forward_arg,
            &remote,
        ])
        .spawn()
        .unwrap_or_else(|e| {
            eprintln!("[!] Failed to start ssh: {}", e);
            eprintln!("    Make sure OpenSSH is installed and in PATH.");
            std::process::exit(1);
        });

    println!("[*] SSH PID: {}. Waiting for tunnel...", ssh.id());

    if wait_for_local_port(TUNNEL_BIND, TUNNEL_PORT, TUNNEL_TIMEOUT_SECS) {
        println!(
            "[+] Tunnel is live on {}:{}",
            TUNNEL_BIND, TUNNEL_PORT
        );
    } else {
        eprintln!(
            "[!] Port {}:{} did not open within {}s.",
            TUNNEL_BIND, TUNNEL_PORT, TUNNEL_TIMEOUT_SECS
        );
        eprintln!("    Check your SSH credentials and that the VPS is reachable.");
        ssh.kill().ok();
        std::process::exit(1);
    }

    println!();
    println!("-------------------------------------------------------------");
    println!("  On your VPS, run:  codex login");
    println!("  Then paste the auth URL printed by Codex below.");
    println!("  (Kill any stale codex processes first: pkill -f codex)");
    println!("-------------------------------------------------------------");
    println!();
    print!("Auth URL: ");
    io::stdout().flush().unwrap();

    let url = read_line();
    let url = url.trim();

    if url.starts_with("http://") || url.starts_with("https://") {
        println!("[*] Opening URL in default browser...");
        open_in_browser(url);
        println!("[+] Browser launched. Complete sign-in, then come back here.");
    } else {
        println!("[!] Input doesn't look like a URL — open it manually.");
        println!("    URL received: {:?}", url);
    }

    println!();
    println!("[*] Press Enter once authentication is complete to shut down the tunnel.");
    read_line();

    println!("[*] Closing SSH tunnel (PID {})...", ssh.id());
    ssh.kill().ok();
    let _ = ssh.wait();
    println!("[+] Done!");
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn get_vps_host() -> String {
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        return args[1].clone();
    }
    print!("Enter VPS host or IP (e.g. 203.0.113.42): ");
    io::stdout().flush().unwrap();
    read_line().trim().to_string()
}

fn read_line() -> String {
    io::stdin()
        .lock()
        .lines()
        .next()
        .unwrap_or(Ok(String::new()))
        .unwrap_or_default()
}

/// Poll localhost:port once per second until it accepts a TCP connection.
fn wait_for_local_port(host: &str, port: u16, timeout_secs: u64) -> bool {
    let addr = format!("{}:{}", host, port);
    for i in 1..=timeout_secs {
        thread::sleep(Duration::from_secs(1));
        if TcpStream::connect(&addr).is_ok() {
            return true;
        }
        print!("\r[*] Still waiting... ({}/{}s)", i, timeout_secs);
        io::stdout().flush().unwrap();
    }
    println!();
    false
}

/// Use `cmd /c start` to open a URL in the default Windows browser.
fn open_in_browser(url: &str) {
    // `start` requires an empty title arg when the target contains special chars.
    Command::new("cmd")
        .args(["/c", "start", "", url])
        .spawn()
        .unwrap_or_else(|e| {
            eprintln!("[!] Could not open browser: {}", e);
            eprintln!("    Open this URL manually: {}", url);
            std::process::exit(1);
        });
}