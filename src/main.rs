#![feature(iter_next_chunk)]

use std::fs::File;
use std::io::{BufRead, BufReader};

use anyhow::{Context, Result};

fn main() -> Result<()> {
    let prof_path = std::env::args().nth(1).context("missing profile data file")?;
    let sym_path = std::env::args().nth(2).context("missing symbol file")?;
    let options = std::env::args().nth(3).context("missing options")?;

    // Assumes invariant TSC
    let cpu_freq_ghz: f64 = std::env::args().nth(4).context("missing CPU freq")?.parse()?;

    let opt_show_offset_from_sym = options.contains('o');
    let opt_expand_offset_from_sym = options.contains('x');
    let opt_ignore_oob_symbol = options.contains('i');

    let mut symbols = BufReader::new(File::open(sym_path)?).lines().filter_map(|line| {
        let line = match line {
            Ok(line) => line,
            Err(err) => return Some(Err(err).context("failed to read line")),
        };

        let mut components = line.splitn(4, ' ');
        let addr = u64::from_str_radix(components.next().expect("malformed data"), 16).expect("malformed data");
        let Ok(len) = u64::from_str_radix(components.next()?, 16) else {
            eprintln!("skipping symbol line `{line}`");
            return None;
        };
        let ty = components.next()?;
        let name = components.next()?;

        if !matches!(ty, "t" | "T") {
            return None;
        }

        Some(Ok((addr, len, name.to_owned())))
    }).collect::<Result<Vec<_>>>()?;
    symbols.sort_by_key(|(addr, _, _)| *addr);

    for line_res in BufReader::new(File::open(prof_path)?).lines() {
        let line: String = line_res.context("failed to read file")?;
        let mut components = line.split(' ');

        let Some(cpu_str) = components.next() else {
            eprintln!("skipping line `{line}`");
            continue;
        };
        let Some(rip_str) = components.next() else {
            eprintln!("skipping line `{line}`");
            continue;
        };
        let Some(tsc_str) = components.next() else {
            eprintln!("skipping line `{line}`");
            continue;
        };

        let Ok(cpu) = u32::from_str_radix(cpu_str, 16) else {
            eprintln!("skipping line `{line}`");
            continue;
        };
        let Ok(rip) = u64::from_str_radix(rip_str, 16) else {
            eprintln!("skipping line `{line}`");
            continue;
        };
        let Ok(tsc) = u64::from_str_radix(tsc_str, 16) else {
            eprintln!("skipping line `{line}`");
            continue;
        };

        let time = (tsc as f64) / (cpu_freq_ghz * 1_000_000_000.0);
        let secs = time as u64;
        let micros = (time.fract() * 1_000_000.0) as u32;

        println!("kernel {cpu} {secs}.{micros}:");

        // TODO: avoid panic
        let callstack = Some(rip).into_iter().chain(components.map(|c| u64::from_str_radix(c, 16).expect("malformed call stack")));

        for addr in callstack {
            let symbol = match symbols.binary_search_by_key(&addr, |(a, _, _)| *a) {
                Ok(i) => Some(&symbols[i]),
                Err(0) => None,
                Err(i) => Some(&symbols[i - 1]),
            };
            if let Some((symbol_addr, symbol_len, symbol_name)) = symbol {
                let off = addr - symbol_addr;

                if opt_ignore_oob_symbol && off >= *symbol_len {
                    continue;
                }

                if opt_expand_offset_from_sym && off != 0 {
                    println!("\t{addr:x} [{symbol_name}+{off}] ([kernel])");
                    println!("\t{symbol_addr:x} [{symbol_name}] ([kernel])");
                } else if opt_show_offset_from_sym {
                    println!("\t{addr:x} [{symbol_name}+{off}] ([kernel])");
                } else {
                    println!("\t{addr:x} [{symbol_name}] ([kernel])");
                }
            } else {
                println!("\t{addr:x} [unknown] ([kernel])");
            }
        }
        println!();
    }

    Ok(())
}
