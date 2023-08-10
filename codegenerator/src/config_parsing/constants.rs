use tokio::time::Duration;

pub struct SyncConfigDefaults {
    pub initial_block_interval: u32,
    pub backoff_multiplicative: f32,
    pub acceleration_additive: u32,
    pub interval_ceiling: u32,
    pub backoff_millis: u32,
    pub query_timeout_millis: u32,
}

pub const SYNC_CONFIG: SyncConfigDefaults = SyncConfigDefaults {
    initial_block_interval: 10_000,
    backoff_multiplicative: 0.8,
    acceleration_additive: 2_000,
    interval_ceiling: 10_000,
    backoff_millis: 5000,
    query_timeout_millis: 20_000,
};

pub const RESERVED_WORDS: &[&str] = &[
    // JavaScript reserved words
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "new",
    "null",
    "return",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
    "yield",
    // ReScript reserved words
    "and",
    "as",
    "assert",
    "asr",
    "begin",
    "constraint",
    "done",
    "downto",
    "end",
    "exception",
    "external",
    "fun",
    "functor",
    "include",
    "inherit",
    "initializer",
    "land",
    "lazy",
    "let",
    "lor",
    "lsl",
    "lsr",
    "lxor",
    "match",
    "method",
    "mod",
    "module",
    "mutable",
    "nonrec",
    "object",
    "of",
    "open",
    "or",
    "pri",
    "pub",
    "rec",
    "sig",
    "struct",
    "then",
    "to",
    "type",
    "val",
    "virtual",
    "when",
    // Typescript Reserved Words
    "any",
    "boolean",
    "constructor",
    "declare",
    "from",
    "get",
    "implements",
    "interface",
    "let",
    "module",
    "number",
    "of",
    "package",
    "private",
    "protected",
    "public",
    "require",
    "set",
    "static",
    "string",
    "symbol",
];

// maximum backoff period for fetching files from IPFS
pub const MAXIMUM_BACKOFF: Duration = Duration::from_secs(32);