use anyhow::{Context, Result};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, BufWriter, Write};

#[derive(Clone, Debug, Default)]
struct Tree {
    bytes_exactly_here: u64,
    bytes_here_and_below: u64,
    children: HashMap<String, Tree>,
}
impl Tree {
    fn add(&mut self, mut path: impl Iterator<Item = String>, len: u64) {
        self.bytes_here_and_below += len;
        match path.next() {
            Some(next) => self.children.entry(next).or_default().add(path, len),
            None => {
                self.bytes_exactly_here += len;
            }
        }
    }
    fn write(&self, prefix: &str, path_prefix: &str, output: &mut impl Write) -> Result<()> {
        let here = self.bytes_exactly_here;

        if here > 0 {
            writeln!(output, "{prefix} {here}")?;
        }

        for (name, child) in &self.children {
            if prefix.is_empty() {
                //child.write(&format!("[{name}]"), output)?;
                child.write(&format!("[{name}]"), &format!("{name}"), output)?;
            } else {
                //child.write(&format!("{prefix};[{prefix}::{name}]"), output)?;
                let next_path_prefix = format!("{path_prefix}::{name}");
                child.write(
                    &format!("{prefix};[{next_path_prefix}]"),
                    &next_path_prefix,
                    output,
                )?;
            }
        }

        Ok(())
    }
}
#[derive(Debug)]
struct Options {
    include_nontext: bool,
}

fn parse(tree: &mut Tree, line: &str, opts: &Options) -> Result<()> {
    let mut components = line.splitn(4, ' ');
    let _start = components
        .next()
        .and_then(|n| u64::from_str_radix(n, 16).ok())
        .context("no start")?;
    let length = components
        .next()
        .and_then(|n| u64::from_str_radix(n, 16).ok())
        .context("no length")?;
    let kind = components.next().context("no type")?;

    if !opts.include_nontext && !matches!(kind, "t" | "T") {
        return Ok(());
    }

    let name = components.next().context("no name")?;
    let tokens = name
        .parse::<proc_macro2::TokenStream>()
        .ok()
        .context("failed to parse name")?;
    // TODO: parsing is very limited, for example unaware of generics and potentially deeper qself
    // patterns
    let syn::ExprPath { qself, path, .. } = syn::parse2::<syn::ExprPath>(tokens)?;

    //eprintln!("QSELF {qself:?} PATH {path:?}");
    let mut segs = Vec::new();

    if let Some(qs) = qself
        && let syn::Type::Path(syn::TypePath { path, .. }) = *qs.ty
    {
        segs.extend(path.segments.iter().map(|s| s.ident.to_string()));
    }
    segs.extend(path.segments.iter().map(|s| s.ident.to_string()));

    if opts.include_nontext
        && !matches!(kind, "t" | "T")
        && let Some(last) = segs.last_mut()
    {
        last.push_str(&format!("@{kind}"));
    }

    tree.add(segs.into_iter(), length);

    Ok(())
}

fn main() -> Result<()> {
    if let Some(arg) = std::env::args().nth(1)
        && matches!(&*arg, "-h" | "--help")
    {
        eprintln!(
            "Usage:\n\tnm -CS /path/to/kernel | redox_sizeprof | inferno-flamegraph >/path/to/flamegraph.svg"
        );
        return Ok(());
    }

    let mut tree = Tree::default();

    let options = Options {
        include_nontext: std::env::var("SIZEPROF_INCLUDE_NONTEXT").map_or(false, |v| v == "1"),
    };
    eprintln!("OPTIONS: {options:?}");

    for line_res in BufReader::new(std::io::stdin()).lines() {
        let line = line_res?;

        match parse(&mut tree, &line, &options) {
            Ok(()) => (),
            Err(_) => continue,
        }
    }
    assert_eq!(tree.bytes_exactly_here, 0, "root node cannot have bytes");

    //std::fs::write("tree-dbg.txt", format!("{tree:?}")).unwrap();

    let mut output = BufWriter::new(std::io::stdout());
    tree.write("", "", &mut output)
        .context("failed to write output")?;
    Ok(())
}
