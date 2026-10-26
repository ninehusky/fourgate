//! Deterministic inventory of recognized binary-level panic transfers.
//!
//! LLVM decodes instructions and resolves DWARF. Rust owns classification and
//! the output schema. This is an inventory, never a proof of panic freedom.

use clap::Args;
use object::{Object, ObjectSection, ObjectSymbol};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Args)]
pub struct MeasurePanics {
    /// Unstripped ELF executable; embedded DWARF enables source attribution.
    #[arg(long)]
    binary: PathBuf,
    /// New directory; refuses to overwrite previous evidence.
    #[arg(long)]
    output: PathBuf,
    /// Directory containing llvm-objdump and llvm-symbolizer (otherwise PATH).
    #[arg(long)]
    llvm_tools: Option<PathBuf>,
}

/// Classification is based on the destination symbol, not guessed from messages.
/// In particular, panic_fmt cannot distinguish panic!, assert!, unreachable!, etc.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
enum PanicKind {
    BoundsCheck,
    SliceRange,
    SliceLengthMismatch,
    UnwrapOrExpect,
    Arithmetic,
    BorrowViolation,
    Assertion,
    PanicMessage,
    PanicRuntime,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum Architecture {
    Aarch64,
    X86_64,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum TransferKind {
    Call,
    TailBranch,
    ConditionalBranch,
}

#[derive(Debug)]
struct Function {
    address: u64,
    end_address: u64,
    /// Aliases share one address and do not multiply the number of sites.
    names: Vec<String>,
}

#[derive(Debug, Serialize)]
struct FunctionSymbol {
    name: String,
    address: u64,
}

#[derive(Debug, Serialize)]
struct Frame {
    function: Option<String>,
    file: Option<String>,
    line: Option<u64>,
    column: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Attribution {
    /// LLVM returns inline frames from innermost to outermost.
    Located {
        frames: Vec<Frame>,
    },
    SymbolsOnly {
        frames: Vec<Frame>,
    },
    Unavailable,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum CrateAttribution {
    /// Source-path attribution is a heuristic, not Cargo package identity.
    SourcePath {
        name: String,
    },
    Unknown,
}

#[derive(Debug, Serialize)]
struct PanicSite {
    address: u64,
    assembly_line: usize,
    instruction: String,
    transfer: TransferKind,
    enclosing_function: Option<FunctionSymbol>,
    target_address: u64,
    target_symbol: String,
    kind: PanicKind,
    attribution: Attribution,
    crate_attribution: CrateAttribution,
}

#[derive(Debug, Serialize)]
struct KindCount {
    kind: PanicKind,
    sites: usize,
}

#[derive(Debug, Serialize)]
struct CrateCount {
    /// None means no source-path attribution was available.
    name: Option<String>,
    sites: usize,
}

#[derive(Debug, Default, Serialize)]
struct Coverage {
    instructions: usize,
    direct_transfers: usize,
    unresolved_indirect_transfers: usize,
    undecoded_instructions: usize,
    excluded_runtime_transfers: usize,
}

#[derive(Debug, Serialize)]
struct Inventory {
    schema_version: u32,
    architecture: Architecture,
    /// Sinks found through the versioned symbol rules below.
    recognized_sinks: usize,
    sites: Vec<PanicSite>,
    by_kind: Vec<KindCount>,
    by_crate: Vec<CrateCount>,
    coverage: Coverage,
}

#[derive(Serialize)]
struct ToolInvocation {
    executable: PathBuf,
    version: String,
    /// Symbolizer is invoked in batches; this records every argv exactly.
    commands: Vec<Vec<String>>,
}

#[derive(Serialize)]
struct Metadata {
    schema_version: u32,
    classifier_version: u32,
    fourgate_version: &'static str,
    binary: PathBuf,
    binary_sha256: String,
    binary_bytes: usize,
    argv: Vec<String>,
    objdump: ToolInvocation,
    symbolizer: ToolInvocation,
}

fn classify(name: &str) -> Option<PanicKind> {
    use PanicKind::*;
    if name.starts_with("core::panicking::panic_bounds_check") {
        Some(BoundsCheck)
    } else if name.starts_with("core::slice::index::slice_") && name.contains("fail")
        || name.starts_with("core::str::slice_error_fail")
    {
        Some(SliceRange)
    } else if name.contains("copy_from_slice") && name.contains("len_mismatch_fail") {
        Some(SliceLengthMismatch)
    } else if name.starts_with("core::option::expect_failed")
        || name.starts_with("core::option::unwrap_failed")
        || name.starts_with("core::result::unwrap_failed")
    {
        Some(UnwrapOrExpect)
    } else if name.starts_with("core::panicking::panic_const::")
        && ["overflow", "zero", "shift"]
            .iter()
            .any(|s| name.contains(s))
    {
        Some(Arithmetic)
    } else if name.starts_with("core::cell::panic_already_") {
        Some(BorrowViolation)
    } else if name.starts_with("core::panicking::assert_failed") {
        Some(Assertion)
    } else if name.starts_with("core::panicking::")
        || name.starts_with("std::panicking::begin_panic")
    {
        Some(PanicMessage)
    } else if name == "rust_begin_unwind"
        || name == "__rustc::rust_begin_unwind"
        || name == "__rustc::__rust_start_panic"
        || name == "__rust_start_panic"
    {
        Some(PanicRuntime)
    } else {
        None
    }
}

fn function_at(functions: &BTreeMap<u64, Function>, address: u64) -> Option<&Function> {
    let (_, function) = functions.range(..=address).next_back()?;
    (address < function.end_address || address == function.address).then_some(function)
}

fn runtime_source(function: &Function) -> bool {
    function.names.iter().any(|name| {
        classify(name).is_some()
            || name.starts_with("std::panicking::rust_panic")
            || name.starts_with("std::panicking::default_hook")
            || name.starts_with("__rustc::rust_panic")
    })
}

fn read_functions(file: &object::File<'_>) -> Result<BTreeMap<u64, Function>> {
    let mut functions = BTreeMap::<u64, Function>::new();
    for symbol in file.symbols() {
        if symbol.kind() != object::SymbolKind::Text || !symbol.is_definition() {
            continue;
        }
        let name = symbol.name()?;
        let name = format!("{:#}", rustc_demangle::demangle(name));
        let function = functions
            .entry(symbol.address())
            .or_insert_with(|| Function {
                address: symbol.address(),
                end_address: symbol.address(),
                names: Vec::new(),
            });
        function.end_address = function.end_address.max(symbol.address() + symbol.size());
        function.names.push(name);
    }
    for function in functions.values_mut() {
        function.names.sort();
        function.names.dedup();
    }
    if functions.is_empty() {
        return Err("no function symbols: provide an unstripped ELF binary".into());
    }
    Ok(functions)
}

/// Parse LLVM's no-raw-bytes output; operands retain registers and comments.
fn instruction(line: &str) -> Option<(u64, &str, &str)> {
    let (address, rest) = line.trim().split_once(':')?;
    let address = u64::from_str_radix(address, 16).ok()?;
    let rest = rest.trim();
    if rest.is_empty() {
        return None;
    }
    let (mnemonic, operands) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
    Some((address, mnemonic, operands.trim()))
}

fn transfer(arch: Architecture, mnemonic: &str) -> Option<TransferKind> {
    use TransferKind::*;
    match arch {
        Architecture::Aarch64 => match mnemonic {
            "bl" | "blr" | "blraa" | "blrab" => Some(Call),
            "b" | "br" | "braa" | "brab" => Some(TailBranch),
            "cbz" | "cbnz" | "tbz" | "tbnz" => Some(ConditionalBranch),
            m if m.starts_with("b.") => Some(ConditionalBranch),
            _ => None,
        },
        Architecture::X86_64 => match mnemonic {
            "call" | "callq" => Some(Call),
            "jmp" | "jmpq" => Some(TailBranch),
            m if m.starts_with('j') || m.starts_with("loop") => Some(ConditionalBranch),
            _ => None,
        },
    }
}

fn direct_target(arch: Architecture, operands: &str) -> Option<u64> {
    // x86 memory-indirect operands may contain a hexadecimal address in a
    // comment. Never mistake that address for a direct destination.
    let operand = match arch {
        Architecture::X86_64 => operands.split_whitespace().next()?,
        Architecture::Aarch64 => operands.split(" <").next()?.rsplit(',').next()?.trim(),
    };
    u64::from_str_radix(operand.strip_prefix("0x")?, 16).ok()
}

fn scan(
    asm: &str,
    arch: Architecture,
    functions: &BTreeMap<u64, Function>,
) -> (Vec<PanicSite>, Coverage, usize) {
    let sinks: BTreeMap<_, _> = functions
        .iter()
        .filter_map(|(&address, function)| {
            function
                .names
                .iter()
                .find_map(|name| classify(name).map(|kind| (address, (name, kind))))
        })
        .collect();
    let mut sites = Vec::new();
    let mut coverage = Coverage::default();
    for (index, line) in asm.lines().enumerate() {
        let Some((address, mnemonic, operands)) = instruction(line) else {
            continue;
        };
        coverage.instructions += 1;
        if mnemonic == "<unknown>" || mnemonic == ".byte" || mnemonic == ".word" {
            coverage.undecoded_instructions += 1;
        }
        let Some(transfer) = transfer(arch, mnemonic) else {
            continue;
        };
        let Some(target) = direct_target(arch, operands) else {
            coverage.unresolved_indirect_transfers += 1;
            continue;
        };
        coverage.direct_transfers += 1;
        let Some((name, kind)) = sinks.get(&target) else {
            continue;
        };
        let enclosing = function_at(functions, address);
        if enclosing.is_some_and(runtime_source) {
            coverage.excluded_runtime_transfers += 1;
            continue;
        }
        sites.push(PanicSite {
            address,
            assembly_line: index + 1,
            instruction: line
                .split_once(':')
                .expect("parsed instruction")
                .1
                .trim()
                .into(),
            transfer,
            enclosing_function: enclosing.and_then(|f| {
                // Aliases identify the same emitted function; use the first
                // sorted symbol name consistently for display.
                f.names.first().map(|name| FunctionSymbol {
                    name: name.clone(),
                    address: f.address,
                })
            }),
            target_address: target,
            target_symbol: (*name).clone(),
            kind: *kind,
            attribution: Attribution::Unavailable,
            crate_attribution: CrateAttribution::Unknown,
        });
    }
    sites.sort_by_key(|s| s.address);
    sites.dedup_by_key(|s| s.address);
    (sites, coverage, sinks.len())
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct LlvmAddress {
    address: String,
    symbol: Vec<LlvmFrame>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct LlvmFrame {
    function_name: String,
    file_name: String,
    line: u64,
    column: u64,
}

fn nonempty(value: String) -> Option<String> {
    (!value.is_empty() && value != "??").then_some(value)
}

fn crate_from_frames(frames: &[Frame]) -> CrateAttribution {
    // The outermost attributable frame owns this emitted instance. Registry
    // versions are retained rather than pretending this is Cargo metadata.
    for frame in frames.iter().rev() {
        if let Some(file) = &frame.file
            && let Some((head, _)) = file.rsplit_once("/src/")
            && let Some(name) = head.rsplit('/').next()
            && !name.is_empty()
        {
            // Cargo git checkouts put the revision, not a crate name, above src.
            if name.len() >= 7 && name.bytes().all(|b| b.is_ascii_hexdigit()) {
                return CrateAttribution::Unknown;
            }
            return CrateAttribution::SourcePath { name: name.into() };
        }
    }
    CrateAttribution::Unknown
}

fn checked_output(tool: &Path, args: &[String]) -> Result<Vec<u8>> {
    let result = Command::new(tool).args(args).output()?;
    if !result.status.success() {
        return Err(format!(
            "{} failed: {}",
            tool.display(),
            String::from_utf8_lossy(&result.stderr)
        )
        .into());
    }
    // Do not silently accept partial decoding/symbolization with warnings.
    if !result.stderr.is_empty() {
        return Err(format!(
            "{} reported: {}",
            tool.display(),
            String::from_utf8_lossy(&result.stderr)
        )
        .into());
    }
    Ok(result.stdout)
}

fn symbolize(tool: &Path, binary: &Path, sites: &mut [PanicSite]) -> Result<Vec<Vec<String>>> {
    let mut commands = Vec::new();
    for chunk in sites.chunks_mut(256) {
        let mut args = vec![
            "--output-style=JSON".into(),
            "--inlines".into(),
            "--demangle".into(),
            format!("--obj={}", binary.display()),
        ];
        args.extend(chunk.iter().map(|s| format!("0x{:x}", s.address)));
        let output = checked_output(tool, &args)?;
        let results: Vec<LlvmAddress> = serde_json::from_slice(&output)?;
        if results.len() != chunk.len() {
            return Err("symbolizer returned the wrong number of addresses".into());
        }
        for (site, result) in chunk.iter_mut().zip(results) {
            if u64::from_str_radix(result.address.trim_start_matches("0x"), 16)? != site.address {
                return Err("symbolizer returned an unexpected address".into());
            }
            let frames: Vec<_> = result
                .symbol
                .into_iter()
                .map(|frame| Frame {
                    function: nonempty(frame.function_name),
                    file: nonempty(frame.file_name),
                    line: (frame.line > 0).then_some(frame.line),
                    column: (frame.column > 0).then_some(frame.column),
                })
                .filter(|frame| frame.function.is_some() || frame.file.is_some())
                .collect();
            site.crate_attribution = crate_from_frames(&frames);
            site.attribution = if frames.iter().any(|f| f.file.is_some() && f.line.is_some()) {
                Attribution::Located { frames }
            } else if !frames.is_empty() {
                Attribution::SymbolsOnly { frames }
            } else {
                Attribution::Unavailable
            };
        }
        commands.push(args);
    }
    Ok(commands)
}

fn tool(directory: &Option<PathBuf>, name: &str) -> PathBuf {
    directory
        .as_ref()
        .map_or_else(|| PathBuf::from(name), |p| p.join(name))
}

fn json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    fs::write(path, bytes)?;
    Ok(())
}

pub fn run(args: MeasurePanics) -> Result<()> {
    let binary = fs::canonicalize(&args.binary)?;
    let bytes = fs::read(&binary)?;
    let file = object::File::parse(bytes.as_slice())?;
    if file.format() != object::BinaryFormat::Elf
        || !matches!(
            file.kind(),
            object::ObjectKind::Executable | object::ObjectKind::Dynamic
        )
    {
        return Err("expected a linked ELF executable (static or PIE)".into());
    }
    let architecture = match file.architecture() {
        object::Architecture::Aarch64 => Architecture::Aarch64,
        object::Architecture::X86_64 => Architecture::X86_64,
        other => return Err(format!("unsupported architecture: {other:?}").into()),
    };
    let functions = read_functions(&file)?;
    let objdump = tool(&args.llvm_tools, "llvm-objdump");
    let symbolizer = tool(&args.llvm_tools, "llvm-symbolizer");
    let objdump_version = String::from_utf8(checked_output(&objdump, &["--version".into()])?)?;
    let symbolizer_version =
        String::from_utf8(checked_output(&symbolizer, &["--version".into()])?)?;
    fs::create_dir(&args.output)?;
    let output = fs::canonicalize(&args.output)?;
    let dump_args = vec![
        "--disassemble".into(),
        "--no-show-raw-insn".into(),
        "--demangle".into(),
        "--print-imm-hex".into(),
        binary.to_string_lossy().into_owned(),
    ];
    let asm = checked_output(&objdump, &dump_args)?;
    fs::write(output.join("disassembly.asm"), &asm)?;
    let (mut sites, coverage, recognized_sinks) =
        scan(std::str::from_utf8(&asm)?, architecture, &functions);
    if coverage.instructions == 0 {
        return Err("disassembler produced no recognized instructions".into());
    }
    let symbolizer_commands = symbolize(&symbolizer, &binary, &mut sites)?;
    let mut kinds = BTreeMap::<PanicKind, usize>::new();
    let mut crates = BTreeMap::<Option<String>, usize>::new();
    for site in &sites {
        *kinds.entry(site.kind).or_default() += 1;
        let name = match &site.crate_attribution {
            CrateAttribution::SourcePath { name } => Some(name.clone()),
            CrateAttribution::Unknown => None,
        };
        *crates.entry(name).or_default() += 1;
    }
    let mut warnings = Vec::new();
    if recognized_sinks == 0 {
        warnings.push("No recognized sinks found; zero detected sites is inconclusive.");
    }
    if !file
        .sections()
        .any(|section| section.name().ok() == Some(".debug_info"))
    {
        warnings.push("No embedded DWARF debug_info; source attribution may be unavailable.");
    }
    let inventory = Inventory {
        schema_version: 3,
        architecture,
        recognized_sinks,
        sites,
        by_kind: kinds
            .into_iter()
            .map(|(kind, sites)| KindCount { kind, sites })
            .collect(),
        by_crate: crates
            .into_iter()
            .map(|(name, sites)| CrateCount { name, sites })
            .collect(),
        coverage,
    };
    json(&output.join("panics.json"), &inventory)?;
    let metadata = Metadata {
        schema_version: 1,
        classifier_version: 1,
        fourgate_version: env!("CARGO_PKG_VERSION"),
        binary,
        binary_sha256: format!("{:x}", Sha256::digest(&bytes)),
        binary_bytes: bytes.len(),
        argv: std::env::args().collect(),
        objdump: ToolInvocation {
            executable: objdump,
            version: objdump_version,
            commands: vec![dump_args],
        },
        symbolizer: ToolInvocation {
            executable: symbolizer,
            version: symbolizer_version,
            commands: symbolizer_commands,
        },
    };
    let located = inventory
        .sites
        .iter()
        .filter(|s| matches!(s.attribution, Attribution::Located { .. }))
        .count();
    let mut summary = format!(
        "# Panic inventory\n\nDetected **{}** direct panic sites; **{located}** have source locations.\n\n| Panic kind | Sites |\n|---|---:|\n",
        inventory.sites.len()
    );
    for count in &inventory.by_kind {
        summary.push_str(&format!(
            "| {} | {} |\n",
            serde_json::to_value(count.kind)?
                .as_str()
                .unwrap_or("unknown"),
            count.sites
        ));
    }
    summary.push_str("\n| Source-path crate | Sites |\n|---|---:|\n");
    for count in &inventory.by_crate {
        summary.push_str(&format!(
            "| {} | {} |\n",
            count.name.as_deref().unwrap_or("unknown"),
            count.sites
        ));
    }
    summary.push_str(&format!("\nRecognized sink symbols: {}. Unresolved indirect transfers: {}. Undecoded instructions: {}.\n\n", inventory.recognized_sinks, inventory.coverage.unresolved_indirect_transfers, inventory.coverage.undecoded_instructions));
    for warning in warnings {
        summary.push_str(&format!("- {warning}\n"));
    }
    summary.push_str("\nAddresses in JSON are virtual addresses, not file offsets. `assembly_line` is one-based. `metadata.json` is written only on successful completion. Build provenance belongs in the companion `fourgate record` directory.\n");
    fs::write(output.join("summary.md"), summary)?;
    if fs::read(&metadata.binary)? != bytes {
        return Err("binary changed during analysis; discard this incomplete report".into());
    }
    json(&output.join("metadata.json"), &metadata)?;
    println!(
        "{}: {} detected panic sites",
        output.display(),
        inventory.sites.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_and_indirect_destinations_are_distinct() {
        assert_eq!(
            direct_target(Architecture::Aarch64, "0x1234 <helper>"),
            Some(0x1234)
        );
        assert_eq!(
            direct_target(Architecture::Aarch64, "x0, #0x3, 0x1234 <helper>"),
            Some(0x1234)
        );
        assert_eq!(direct_target(Architecture::Aarch64, "x16"), None);
        assert_eq!(
            direct_target(Architecture::X86_64, "0x1234 <helper>"),
            Some(0x1234)
        );
        assert_eq!(
            direct_target(Architecture::X86_64, "*0x12(%rip) # 0x1234 <helper>"),
            None
        );
    }

    #[test]
    fn runtime_helpers_and_aliases_do_not_multiply_sites() {
        let functions = [
            Function {
                address: 0x1000,
                end_address: 0x1010,
                names: vec!["app::read".into()],
            },
            Function {
                address: 0x2000,
                end_address: 0x2010,
                names: vec!["alias".into(), "core::panicking::panic_bounds_check".into()],
            },
            Function {
                address: 0x3000,
                end_address: 0x3010,
                names: vec!["core::panicking::panic_fmt".into()],
            },
            Function {
                address: 0x4000,
                end_address: 0x4010,
                names: vec!["std::panicking::set_hook".into()],
            },
        ]
        .into_iter()
        .map(|f| (f.address, f))
        .collect();
        let asm = "1000: bl 0x2000 <bounds>\n1004: br x16\n2000: bl 0x3000 <panic_fmt>\n4000: bl 0x3000 <panic_fmt>\n";
        let (sites, coverage, sinks) = scan(asm, Architecture::Aarch64, &functions);
        assert_eq!(sites.len(), 2);
        assert_eq!(sites[0].kind, PanicKind::BoundsCheck);
        assert_eq!(sinks, 2);
        assert_eq!(coverage.unresolved_indirect_transfers, 1);
        assert_eq!(coverage.excluded_runtime_transfers, 1);
    }
}
