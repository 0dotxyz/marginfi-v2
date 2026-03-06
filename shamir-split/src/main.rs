use anyhow::{anyhow, bail, Context, Result};
use gf256::shamir::shamir;
use std::{env, fs, path::Path};

const N: usize = 3; // total shares
const M: usize = 2; // threshold (any M can recover)
const SECRET_LEN: usize = 64;

// Share line format (one line per share):
// ss1:<m>:<x>:<y_base58>
// where share bytes for gf256 are [x || y...]
const SHARE_PREFIX: &str = "ss1";

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage(&args[0]);
        std::process::exit(2);
    }

    match args[1].as_str() {
        "split" => {
            if args.len() != 3 {
                print_usage(&args[0]);
                std::process::exit(2);
            }
            cmd_split(Path::new(&args[2]))
        }
        "recover" => {
            if args.len() < 2 + M {
                print_usage(&args[0]);
                std::process::exit(2);
            }
            let share_paths: Vec<&Path> = args[2..].iter().map(|s| Path::new(s)).collect();
            cmd_recover(&share_paths)
        }
        _ => {
            print_usage(&args[0]);
            std::process::exit(2);
        }
    }
}

fn print_usage(bin: &str) {
    eprintln!(
        "Usage:\n  {bin} split <path-to-solana-private-key>\n  {bin} recover <share-file-1> <share-file-2> [share-file-3 ...]\n\nDefaults:\n  N={N} M={M}\n"
    );
}

fn cmd_split(path: &Path) -> Result<()> {
    let secret = parse_solana_secret_file(path)?;
    if secret.len() != SECRET_LEN {
        bail!(
            "expected secret length {} bytes, got {}",
            SECRET_LEN,
            secret.len()
        );
    }

    let shares = shamir::generate(&secret, N, M);

    println!("input_path={}", path.display());
    println!("secret_len_bytes={}", secret.len());
    println!("N={} M={}", N, M);
    println!();

    for (i, sh) in shares.iter().enumerate() {
        let line = encode_share_line(sh)?;
        println!("{:02}: {}", i + 1, line);
    }

    Ok(())
}

fn cmd_recover(share_paths: &[&Path]) -> Result<()> {
    let mut shares: Vec<Vec<u8>> = Vec::with_capacity(share_paths.len());
    let mut expected_m: Option<usize> = None;

    for p in share_paths {
        let raw = fs::read_to_string(p)
            .with_context(|| format!("failed to read share file {}", p.display()))?;
        let line = raw.trim();
        if line.is_empty() {
            bail!("share file {} is empty", p.display());
        }

        let (m, share_bytes) = decode_share_line(line)
            .with_context(|| format!("failed to parse share in {}", p.display()))?;

        if let Some(em) = expected_m {
            if m != em {
                bail!("share {} has M={} but earlier share had M={}", p.display(), m, em);
            }
        } else {
            expected_m = Some(m);
        }

        shares.push(share_bytes);
    }

    let m = expected_m.unwrap_or(M);
    if shares.len() < m {
        bail!("not enough shares: provided {}, need at least {}", shares.len(), m);
    }

    {
        use std::collections::HashSet;
        let mut seen = HashSet::new();
        for sh in &shares {
            if sh.len() != 1 + SECRET_LEN {
                bail!(
                    "unexpected share byte length {} (expected {})",
                    sh.len(),
                    1 + SECRET_LEN
                );
            }
            let x = sh[0];
            if !seen.insert(x) {
                bail!("duplicate share x-coordinate detected: {}", x);
            }
        }
    }

    let secret = shamir::reconstruct(&shares);

    if secret.len() != SECRET_LEN {
        bail!(
            "recovered secret length {} != expected {}",
            secret.len(),
            SECRET_LEN
        );
    }

    let json = serde_json::to_string(&secret).context("failed to encode JSON")?;
    println!("{}", json);

    Ok(())
}

fn encode_share_line(share: &[u8]) -> Result<String> {
    if share.len() != 1 + SECRET_LEN {
        bail!(
            "unexpected share length: got {}, expected {}",
            share.len(),
            1 + SECRET_LEN
        );
    }
    let x = share[0];
    let y = &share[1..];
    let y_b58 = bs58::encode(y).into_string();
    Ok(format!("{}:{}:{}:{}", SHARE_PREFIX, M, x, y_b58))
}

fn decode_share_line(line: &str) -> Result<(usize, Vec<u8>)> {
    let trimmed = line.trim();
    let maybe = trimmed.splitn(2, ": ").collect::<Vec<_>>();
    let core = if maybe.len() == 2 && maybe[0].chars().all(|c| c.is_ascii_digit()) {
        maybe[1].trim()
    } else {
        trimmed
    };

    let parts: Vec<&str> = core.split(':').collect();
    if parts.len() != 4 {
        bail!("invalid share format");
    }
    if parts[0] != SHARE_PREFIX {
        bail!("invalid share prefix");
    }

    let m: usize = parts[1].parse().context("invalid m")?;
    let x_u: u8 = parts[2]
        .parse::<u16>()
        .context("invalid x")?
        .try_into()
        .map_err(|_| anyhow!("x out of range"))?;

    if m < 2 {
        bail!("invalid m {}", m);
    }
    if x_u == 0 {
        bail!("x must be in 1..=255");
    }

    let y = bs58::decode(parts[3])
        .into_vec()
        .map_err(|e| anyhow!("base58 decode failed: {e}"))?;

    if y.len() != SECRET_LEN {
        bail!("share y length {} != {}", y.len(), SECRET_LEN);
    }

    let mut share = Vec::with_capacity(1 + y.len());
    share.push(x_u);
    share.extend_from_slice(&y);

    Ok((m, share))
}

fn is_hex(s: &str) -> bool {
    let t = s.trim();
    let t = t.strip_prefix("0x").unwrap_or(t);
    !t.is_empty() && t.len() % 2 == 0 && t.chars().all(|c| c.is_ascii_hexdigit())
}

fn hex_to_bytes(s: &str) -> Result<Vec<u8>> {
    let t = s.trim();
    let t = t.strip_prefix("0x").unwrap_or(t);
    if t.len() % 2 != 0 {
        bail!("hex string must have even length");
    }
    let mut out = Vec::with_capacity(t.len() / 2);
    for i in (0..t.len()).step_by(2) {
        let byte = u8::from_str_radix(&t[i..i + 2], 16)
            .with_context(|| format!("invalid hex at {}", i))?;
        out.push(byte);
    }
    Ok(out)
}

fn parse_solana_secret_file(path: &Path) -> Result<Vec<u8>> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let s = raw.trim();

    if s.starts_with('[') {
        let vals: Vec<u64> = serde_json::from_str(s).context("failed to parse JSON keypair")?;
        if vals.len() != SECRET_LEN {
            bail!("unexpected JSON key length {}", vals.len());
        }

        let mut bytes = Vec::with_capacity(vals.len());
        for (i, v) in vals.into_iter().enumerate() {
            if v > 255 {
                bail!("byte out of range at index {}: {}", i, v);
            }
            bytes.push(v as u8);
        }
        return Ok(bytes);
    }

    if is_hex(s) {
        let bytes = hex_to_bytes(s)?;
        if bytes.len() != SECRET_LEN {
            bail!("unexpected hex key length {}", bytes.len());
        }
        return Ok(bytes);
    }

    let bytes = bs58::decode(s)
        .into_vec()
        .map_err(|e| anyhow!("base58 decode failed: {e}"))?;

    if bytes.len() != SECRET_LEN {
        bail!("unexpected base58 key length {}", bytes.len());
    }

    Ok(bytes)
}