/*
* Bind Manager; a CLI tool to manage BIND blacklisted zones.
* Copyright (c) 2024 TheFinnaCompany Ltd
*/

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use structopt::StructOpt;
use serde::{Deserialize, Serialize};

#[derive(StructOpt)]
#[structopt(name = "bind_manager", about = "A CLI tool to manage BIND blacklisted zones.")]
enum Cli {
    Add {
        #[structopt(help = "The domain to be added.")]
        domain: String,
        #[structopt(help = "The reason for blacklisting.", default_value = "No reason provided.")]
        reason: String,
    },
    Del {
        #[structopt(help = "The index of the domain to be removed.")]
        index: usize,
    },
    List,
    About
}

const ZONES_FILE_PATH: &str = "/etc/bind/blacklisted.zones";
const REASON_LOG_PATH: &str = "/etc/bind/reason_log.json";

#[derive(Serialize, Deserialize, Debug)]
struct DomainEntry {
    domain: String,
    reason: String,
}

fn main() -> io::Result<()> {
    let args = Cli::from_args();

    match args {
        Cli::Add { domain, reason } => add_domain(&domain, &reason)?,
        Cli::Del { index } => remove_domain(index)?,
        Cli::List => list_domains()?,
        Cli::About => about(),
    }

    Ok(())
}

fn about() {
    let top_heading = format!("--- {} v{} ---", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"));
    println!("{}", top_heading);
    println!("This tool was created to aid in managing BIND blacklisted zones - making it easier to add, remove, and list domains that are blocked by the DNS server.\nIt's meant to be simple and efficient, and it uses a JSON file to store the reasons for blacklisting domains.");
    println!("\nAuthors: {}", env!("CARGO_PKG_AUTHORS").split(':').collect::<Vec<&str>>().join(", "));
    println!("{}", top_heading.chars().map(|_| "-").collect::<String>());
}

fn load_reason_log() -> io::Result<Vec<DomainEntry>> {
    if Path::new(REASON_LOG_PATH).exists() {
        let file = fs::File::open(REASON_LOG_PATH)?;
        let reader = BufReader::new(file);
        match serde_json::from_reader(reader) {
            Ok(entries) => Ok(entries),
            Err(_) => Ok(Vec::new()),
        }
    } else {
        Ok(Vec::new())
    }
}

fn save_reason_log(entries: &Vec<DomainEntry>) -> io::Result<()> {
    let file = OpenOptions::new().write(true).truncate(true).create(true).open(REASON_LOG_PATH)?;
    serde_json::to_writer(file, &entries)?;
    Ok(())
}

fn add_domain(domain: &str, reason: &str) -> io::Result<()> {
    let mut entries = load_reason_log()?;

    if entries.iter().any(|entry| entry.domain == domain) {
        println!("Domain {} is already in the blacklist.", domain);
        return Ok(());
    }

    let entry = format!("zone \"{}\" {{type master; file \"/etc/bind/zones/master/blockeddomains.db\";}};\n\n", domain);

    let mut file = OpenOptions::new().append(true).open(ZONES_FILE_PATH)?;
    file.write_all(entry.as_bytes())?;

    entries.push(DomainEntry { domain: domain.to_string(), reason: reason.to_string() });
    save_reason_log(&entries)?;

    println!("Domain {} added to blacklist.", domain);
    Ok(())
}

fn remove_domain(index: usize) -> io::Result<()> {
    let mut entries = load_reason_log()?;
    if index >= entries.len() {
        println!("Index out of bounds.");
        return Ok(());
    }

    let domain = entries[index].domain.clone(); // Clone the domain to avoid borrowing issues
    entries.remove(index);
    save_reason_log(&entries)?;

    let path = Path::new(ZONES_FILE_PATH);
    let file = OpenOptions::new().read(true).open(&path)?;
    let reader = BufReader::new(file);

    let lines: Vec<String> = reader
        .lines()
        .filter_map(|line| line.ok())
        .filter(|line| !line.contains(&domain)) // Use the cloned domain
        .collect();

    fs::write(&path, lines.join("\n"))?;

    println!("Domain {} removed from blacklist.", domain);
    Ok(())
}

fn list_domains() -> io::Result<()> {
    // Load the domain entries and their reasons from the JSON file
    let entries = load_reason_log()?;
    let mut reasons_map = HashMap::new();
    for entry in entries {
        reasons_map.insert(entry.domain.clone(), entry.reason);
    }

    // Read the zones file and collect domains
    let file = fs::File::open(ZONES_FILE_PATH)?;
    let reader = BufReader::new(file);
    let mut listed_domains = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if let Some(domain) = parse_domain_from_line(&line) {
            listed_domains.push(domain);
        }
    }

    // Sort domains alphabetically
    listed_domains.sort();

    // Print the domains with reasons
    for (i, domain) in listed_domains.iter().enumerate() {
        println!("{} {} - {}", i, domain, reasons_map.get(domain).unwrap_or(&"No reason provided.".to_string()));
    }

    Ok(())
}

fn parse_domain_from_line(line: &str) -> Option<String> {
    // A simple parser for the domain in the line. Adjust regex as needed.
    let parts: Vec<&str> = line.split_whitespace().collect();
    if let Some(part) = parts.get(1) {
        if part.starts_with('"') && part.ends_with('"') {
            return Some(part.trim_matches('"').to_string());
        }
    }
    None
}