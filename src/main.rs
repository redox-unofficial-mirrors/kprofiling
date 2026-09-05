use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};

use anyhow::{Context, Result};

fn parse_symbols(path: &str) -> Result<Vec<(u64, u64, String)>> {
    let mut symbols = BufReader::new(File::open(path)?)
        .lines()
        .filter_map(|line| {
            let line = match line {
                Ok(line) => line,
                Err(err) => return Some(Err(err).context("failed to read line")),
            };

            let mut components = line.splitn(4, ' ');
            let addr = u64::from_str_radix(components.next().expect("malformed data"), 16).ok()?;
            //.expect("malformed data");
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
        })
        .collect::<Result<Vec<_>>>()?;
    symbols.sort_by_key(|(addr, _, _)| *addr);
    Ok(symbols)
}

fn main() -> Result<()> {
    let prof_path = std::env::args()
        .nth(1)
        .context("missing profile data file")?;
    let ksym_path = std::env::args().nth(2).context("missing symbol file")?;
    let options = std::env::args().nth(3).context("missing options")?;
    // Assumes invariant TSC
    let cpu_freq_ghz: f64 = std::env::args()
        .nth(4)
        .context("missing CPU freq")?
        .parse()?;

    let opt_show_offset_from_sym = options.contains('o');
    let opt_expand_offset_from_sym = options.contains('x');
    let opt_ignore_oob_symbol = options.contains('i');
    let opt_include_unspecified_contexts = options.contains('u');
    let opt_include_nonkernel_chain = options.contains('U');

    let kernel_symbols = parse_symbols(&ksym_path).context("failed to parse kernel symbols")?;
    let all_user_symbols = if let Some(usym_paths_file) = std::env::args().nth(5) {
        let file_list =
            BufReader::new(File::open(usym_paths_file).context("failed to open symbol file list")?)
                .lines()
                .map(|line| {
                    let line = line?;
                    let mut i = line.trim_end_matches("\n").splitn(2, '=');
                    let key = i.next().context("malformed")?.to_owned();
                    let value = i.next().context("malformed")?.to_owned();
                    Ok((key, value))
                })
                .collect::<Result<Vec<_>>>()?;

        file_list
            .into_iter()
            .map(|(key, file)| Ok((key, parse_symbols(&file)?)))
            .collect::<Result<HashMap<_, _>>>()
            .context("failed to read user symbols file")?
    } else {
        HashMap::new()
    };

    for line_res in BufReader::new(File::open(prof_path)?).lines() {
        let line: String = line_res.context("failed to read file")?;
        let mut components = line.split(' ');

        let Some(context_str) = components.next() else {
            eprintln!("skipping line `{line}`");
            continue;
        };
        let Some(cpu_str) = components.next() else {
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
        let Ok(tsc) = u64::from_str_radix(tsc_str, 16) else {
            eprintln!("skipping line `{line}`");
            continue;
        };

        let time = (tsc as f64) / (cpu_freq_ghz * 1_000_000_000.0);
        let secs = time as u64;
        let micros = (time.fract() * 1_000_000.0) as u32;

        // TODO: avoid panic
        let callstack = components
            .map(|c| u64::from_str_radix(c, 16).expect("malformed call stack"))
            .collect::<Vec<_>>();

        let from_userspace = callstack.get(0).is_some_and(|addr| addr & (1 << 63) == 0);

        let these_user_symbols = match all_user_symbols.get(context_str) {
            Some(syms) => syms,
            None => {
                if from_userspace && context_str == "[kmain]" {
                    eprintln!("from userspace and kmain");
                }
                // TODO: find places where from_userspace && kmain...
                if !from_userspace || context_str == "[kmain]" {
                    &Vec::new()
                } else if opt_include_unspecified_contexts {
                    eprintln!("Context `{context_str}` has no symbols, keeping userspace sample");
                    &Vec::new()
                } else {
                    eprintln!("Context `{context_str}` has no symbols, ignoring userspace sample");
                    continue;
                }
            }
        };

        if !opt_include_nonkernel_chain && from_userspace {
            continue;
        }
        // TODO: which one?
        if opt_include_nonkernel_chain {
            println!("{context_str} {cpu} {secs}.{micros}:");
        } else {
            println!("kernel {cpu} {secs}.{micros}:");
        }

        for addr in callstack {
            let addr_is_userspace_not_kernel = addr & (1 << 63) == 0;

            if !opt_include_nonkernel_chain && addr_is_userspace_not_kernel {
                // just stop iterating when reaching a user-kernel boundary
                break;
            }

            let (symbols, environment) = if addr_is_userspace_not_kernel {
                (these_user_symbols, context_str)
            } else {
                (&kernel_symbols, "[kernel]")
            };

            let symbol = match symbols.binary_search_by_key(&addr, |(a, _, _)| *a) {
                Ok(i) => Some(&symbols[i]),
                Err(0) => None,
                Err(i) => Some(&symbols[i - 1]),
            };
            if let Some((symbol_addr, symbol_len, symbol_name)) = symbol {
                let off = addr - symbol_addr;

                if opt_ignore_oob_symbol && off >= *symbol_len {
                    //continue;
                    break;
                }

                if opt_expand_offset_from_sym && off != 0 {
                    println!("\t{addr:x} [{symbol_name}+{off:#0x}] ({environment})");
                    println!("\t{symbol_addr:x} [{symbol_name}] ({environment})");
                } else if opt_show_offset_from_sym {
                    println!("\t{addr:x} [{symbol_name}+{off:#0x}] ({environment})");
                } else {
                    println!("\t{addr:x} [{symbol_name}] ({environment})");
                }
            } else {
                println!("\t{addr:x} [unknown] ({environment})");
            }
        }
        println!();
    }

    Ok(())
}
