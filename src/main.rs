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

    let remote = get_remote();

    let forward_arg = format!(
        "{}:{}:{}:{}",
        TUNNEL_BIND, TUNNEL_PORT, TUNNEL_BIND, TUNNEL_PORT
    );

    println!(
        "[*] Spawning tunnel: localhost:{} -> {}:{}",
        TUNNEL_PORT,
        remote,
        TUNNEL_PORT
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
        println!("[+] Tunnel is live on {}:{}", TUNNEL_BIND, TUNNEL_PORT);
    } else {
        eprintln!(
            "[!] Port {}:{} did not open within {}s.",
            TUNNEL_BIND, TUNNEL_PORT, TUNNEL_TIMEOUT_SECS
        );
        eprintln!(
            "    Check your SSH credentials and that the Mac is reachable."
        );
        ssh.kill().ok();
        std::process::exit(1);
    }

    println!();
    println!("-------------------------------------------------------------");
    println!("  On your Mac, run:  codex login");
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
    println!(
        "[*] Press Enter once authentication is complete to shut down the tunnel."
    );
    read_line();

    println!("[*] Closing SSH tunnel (PID {})...", ssh.id());
    ssh.kill().ok();
    let _ = ssh.wait();
    println!("[+] Done!");
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn get_remote() -> String {
    let args: Vec<String> = std::env::args().collect();
    let raw = if args.len() > 1 {
        args[1].clone()
    } else {
        print!("Enter remote in user@ip format (e.g. alice@192.168.1.50): ");
        io::stdout().flush().unwrap();
        read_line().trim().to_string()
    };

    validate_remote_or_die(&raw);
    raw
}

fn validate_remote_or_die(input: &str) {
    let parts: Vec<&str> = input.splitn(2, '@').collect();

    let (user, ip_str) = match parts.as_slice() {
        [u, ip] => (*u, *ip),
        _ => die_bad_input(input, "Missing '@' separator. Expected format: user@ip"),
    };

    if user.is_empty() {
        die_bad_input(input, "Username is empty. Expected format: user@ip");
    }

    if user.contains(' ') {
        die_bad_input(input, "Username contains a space.");
    }

    validate_ipv4_or_die(input, ip_str);
}

fn validate_ipv4_or_die(raw_input: &str, ip_str: &str) {
    let octets: Vec<&str> = ip_str.split('.').collect();

    if octets.len() != 4 {
        die_bad_input(
            raw_input,
            &format!(
                "IP '{}' must have exactly 4 octets (got {}).",
                ip_str,
                octets.len()
            ),
        );
    }

    for (i, octet) in octets.iter().enumerate() {
        if octet.is_empty() {
            die_bad_input(
                raw_input,
                &format!("Octet {} is empty (double dot or trailing dot).", i + 1),
            );
        }

        // Reject leading zeros (e.g. "01", "007") — ambiguous and wrong
        if octet.len() > 1 && octet.starts_with('0') {
            die_bad_input(
                raw_input,
                &format!(
                    "Octet {} ('{}') has a leading zero. Write it as plain digits.",
                    i + 1,
                    octet
                ),
            );
        }

        match octet.parse::<u32>() {
            Ok(n) if n <= 255 => {}
            Ok(n) => die_bad_input(
                raw_input,
                &format!(
                    "Octet {} ('{}') is {}. Each octet must be 0-255.",
                    i + 1,
                    octet,
                    n
                ),
            ),
            Err(_) => die_bad_input(
                raw_input,
                &format!(
                    "Octet {} ('{}') is not a number.",
                    i + 1,
                    octet
                ),
            ),
        }
    }
}

fn die_bad_input(input: &str, reason: &str) -> ! {
    eprintln!();
    eprintln!("╔══════════════════════════════════════════════════════════╗");
    eprintln!("║                  INVALID INPUT — ABORTING                ║");
    eprintln!("╚══════════════════════════════════════════════════════════╝");
    eprintln!();
    eprintln!("  You entered : {:?}", input);
    eprintln!("  Problem     : {}", reason);
    eprintln!();
    eprintln!("  Expected    : user@ip   e.g.  alice@192.168.1.50");
    eprintln!("  IP rules    : exactly 4 numeric octets, each 0-255,");
    eprintln!("                separated by dots, no leading zeros.");
    eprintln!();
    std::process::exit(1);
}

fn read_line() -> String {
    io::stdin()
        .lock()
        .lines()
        .next()
        .unwrap_or(Ok(String::new()))
        .unwrap_or_default()
}

fn wait_for_local_port(host: &str, port: u16, timeout_secs: u64) -> bool {
    let addr = format!("{}:{}", host, port);
    for i in 1..=timeout_secs {
        thread::sleep(Duration::from_secs(1));
        if TcpStream::connect(&addr).is_ok() {
            return true;
        }
        print!("\r[*] Still waiting... ({}/{}s)  ", i, timeout_secs);
        io::stdout().flush().unwrap();
    }
    println!();
    false
}

fn open_in_browser(url: &str) {
    Command::new("cmd")
        .args(["/c", "start", "", url])
        .spawn()
        .unwrap_or_else(|e| {
            eprintln!("[!] Could not open browser: {}", e);
            eprintln!("    Open this URL manually: {}", url);
            std::process::exit(1);
        });
}