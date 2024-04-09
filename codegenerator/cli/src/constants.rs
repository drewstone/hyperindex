pub mod project_paths {
    pub const DEFAULT_PROJECT_ROOT_PATH: &str = ".";
    pub const DEFAULT_GENERATED_PATH: &str = "generated/";
    pub const DEFAULT_CONFIG_PATH: &str = "config.yaml";
    pub const ESBUILD_PATH: &str = "esbuild-handlers";
}

pub mod links {
    pub const DOC_CONFIGURATION_FILE: &str = "https://docs.envio.dev/docs/configuration-file";
}

pub mod reserved_keywords {
    pub const JAVASCRIPT_RESERVED_WORDS: &[&str] = &[
        "abstract",
        "arguments",
        "await",
        "boolean",
        "break",
        "byte",
        "case",
        "catch",
        "char",
        "class",
        "const",
        "continue",
        "debugger",
        "default",
        "delete",
        "do",
        "double",
        "else",
        "enum",
        "eval",
        "export",
        "extends",
        "false",
        "final",
        "finally",
        "float",
        "for",
        "function",
        "goto",
        "if",
        "implements",
        "import",
        "in",
        "instanceof",
        "int",
        "interface",
        "let",
        "long",
        "native",
        "new",
        "null",
        "package",
        "private",
        "protected",
        "public",
        "return",
        "short",
        "static",
        "super",
        "switch",
        "synchronized",
        "this",
        "throw",
        "throws",
        "transient",
        "true",
        "try",
        "typeof",
        "var",
        "void",
        "volatile",
        "while",
        "with",
        "yield",
    ];

    pub const TYPESCRIPT_RESERVED_WORDS: &[&str] = &[
        "any",
        "as",
        "boolean",
        "break",
        "case",
        "catch",
        "class",
        "const",
        "constructor",
        "continue",
        "declare",
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
        "from",
        "function",
        "get",
        "if",
        "implements",
        "import",
        "in",
        "instanceof",
        "interface",
        "let",
        "module",
        "new",
        "null",
        "number",
        "of",
        "package",
        "private",
        "protected",
        "public",
        "require",
        "return",
        "set",
        "static",
        "string",
        "super",
        "switch",
        "symbol",
        "this",
        "throw",
        "true",
        "try",
        "type",
        "typeof",
        "var",
        "void",
        "while",
        "with",
        "yield",
    ];

    pub const RESCRIPT_RESERVED_WORDS: &[&str] = &[
        "and",
        "as",
        "assert",
        "constraint",
        "else",
        "exception",
        "external",
        "false",
        "for",
        "if",
        "in",
        "include",
        "lazy",
        "let",
        "module",
        "mutable",
        "of",
        "open",
        "rec",
        "switch",
        "true",
        "try",
        "type",
        "when",
        "while",
        "with",
    ];

    pub const ENVIO_INTERNAL_RESERVED_POSTGRES_TYPES: &[&str] = &["EVENT_TYPE", "CONTRACT_TYPE"];
}
