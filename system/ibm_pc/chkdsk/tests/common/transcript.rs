//! The oracle transcripts of `oracle/transcripts/` and the cases they belong to.

use std::path::{Path, PathBuf};

pub struct Transcript {
    pub command: String,
    pub dos_sha: String,
    pub before: String,
    pub after: String,
    pub rows: Vec<String>,
}

pub fn parse(text: &str) -> Transcript {
    let find = |key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(key))
            .unwrap_or_else(|| panic!("no {key:?}"))
            .to_string()
    };
    Transcript {
        command: find("> "),
        dos_sha: find("# dos image sha256 "),
        before: find("# disk B sha256 before "),
        after: find("# disk B sha256 after "),
        rows: text
            .lines()
            .filter_map(|l| l.strip_prefix('|').map(|r| r.trim_end().to_string()))
            .collect(),
    }
}

impl Transcript {
    /// The screen rows between the command typed as `typed` and the next prompt.
    pub fn rows_after(&self, typed: &str) -> Vec<String> {
        let typed = format!("A>{typed}");
        let from = self
            .rows
            .iter()
            .position(|r| *r == typed)
            .unwrap_or_else(|| panic!("no row {typed:?}"))
            + 1;
        let to = from
            + self.rows[from..]
                .iter()
                .position(|r| r == "A>")
                .expect("prompt row");
        self.rows[from..to].to_vec()
    }
}

/// Cases whose disk B is one of `disks.rs`: the transcript name and the disk.
pub const CASES: &[(&str, &str)] = &[
    ("002_blank", "blank"),
    ("004_sizes", "d3_sizes"),
    ("005_size_error", "d4_size_error"),
    ("006_allocation_error", "d5_allocation_error"),
    ("007_cross_linked", "d6_cross_linked"),
    ("008_lost_chain", "d7_lost_chain"),
    ("009_full", "d8_full"),
    ("010_media_byte", "d9_media_byte"),
    ("012_bad_drive", "d3_sizes"),
    ("013_bad_parameter", "d3_sizes"),
    ("014_bad_parameter_name", "d3_sizes"),
    ("015_directory_error", "d10_directory_error"),
    ("016_chain_beyond_disk", "d11_chain_beyond_disk"),
    ("017_chain_loop", "d12_chain_loop"),
    ("018_no_clusters", "d13_no_clusters"),
    ("019_one_lost_cluster", "d14_one_lost_cluster"),
    ("020_chain_to_315", "d15_chain_to_315"),
    ("021_first_cluster_315", "d16_first_cluster_315"),
    ("022_end_ff8", "d17_end_ff8"),
    ("023_chain_ff7", "d18_chain_ff7"),
    ("024_lost_last_cluster", "d19_lost_last_cluster"),
];

pub fn transcript(name: &str) -> Option<Transcript> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("oracle/transcripts")
        .join(format!("{name}.txt"));
    std::fs::read_to_string(path).ok().map(|t| parse(&t))
}

/// The directory with the reference BIOS, BASIC and disks, if there is one.
pub fn reference_dir() -> Option<PathBuf> {
    [
        std::env::var_os("IBM_PC_ROMS").map(PathBuf::from),
        std::env::var_os("AI_HW_ENGINEER")
            .map(|p| PathBuf::from(p).join("system/ibm_pc/work/reference")),
        Some(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../../../ai_hw_engineer/system/ibm_pc/work/reference"),
        ),
    ]
    .into_iter()
    .flatten()
    .find(|p| p.join("pcdos100.img").exists())
}
