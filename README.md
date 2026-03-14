# smartgrep

Structural code navigation for coding agents. Parses source files via tree-sitter and extracts symbols — functions, classes, structs, interfaces, methods, enums — into a queryable index.

The queries are designed to be **readable by humans and generatable by LLMs**, unlike grep regexes.

```bash
# grep — terse, brittle, requires regex expertise to read
grep -rn "^pub\s\+fn\|pub async fn" src/commands/ | grep -v "//\|mod.rs"

# smartgrep — reads like a sentence
smartgrep query "functions where visibility = public and file contains 'commands/'"
```

When Claude uses smartgrep, you can see exactly what it's looking for at a glance.

---

## Installation

### Homebrew (macOS)

```bash
brew tap rohitkg98/smartgrep https://github.com/rohitkg98/homebrew-smartgrep
brew install smartgrep
```

Requires [Rust/cargo](https://www.rust-lang.org/tools/install) (builds from source).

### From source

```bash
git clone git@github.com:rohitkg98/smartgrep.git
cd smartgrep
cargo install --path .
```

Requires Rust 1.70+.

---

## Giving smartgrep to Claude

Install as a Claude Code skill so Claude automatically uses smartgrep for structural questions — no prompting needed:

```bash
smartgrep install-skill --global    # all projects (~/.claude/skills/)
smartgrep install-skill             # this repo only (.claude/skills/)
```

Or add to your project's `CLAUDE.md` manually:

```markdown
## Code Navigation

Use `smartgrep` for structural code queries instead of grep/find.

- `smartgrep query "<dsl>"` for composable one-shot structural questions
- `smartgrep context <file>` for a structural overview of a file
- `smartgrep query "symbol <Name> | with deps, refs"` to understand a symbol's role
- Use batch queries (semicolon-separated) to answer multi-part questions in one call
```

---

## Watching what Claude does

Every smartgrep call is logged. Run this to see Claude's recent queries:

```bash
smartgrep log
```

Example output:

```
ts                   command  args                                                      results  ms
2026-03-14 10:42:11  query    classes where attributes contains '@RestController'            4  12
2026-03-14 10:42:18  query    methods where parent = UserController | with signature         7   9
2026-03-14 10:42:31  query    symbol OrderService | with deps, refs                          1   8
2026-03-14 10:43:02  query    functions where file contains 'commands/' | with params        6  11
```

Each line is one tool call Claude made. Compare these to what you'd have had to decipher with grep:

```bash
# What Claude asked for (smartgrep)              # What you'd have with grep
classes where attributes contains '@RestController'   grep -rn "@RestController" src/ | grep "^class\|^public class"
methods where parent = UserController                 grep -rn "UserController" src/ | grep "^\s\+public\|void\|String"
symbol OrderService | with deps, refs                 grep -rn "OrderService" src/ | grep "import\|extends\|new OrderService"
```

The smartgrep query column tells you exactly what Claude was looking for, in plain language.

---

## Query DSL

The `query` command is the main feature. Queries compose a source, optional filters, and pipeline stages:

```
source [where conditions] [| stage] [| stage] ...
```

### Readability in practice

Side by side with grep, for the same questions:

| What you want to know | grep | smartgrep |
|---|---|---|
| All public functions | `grep -rn "^pub fn\|pub async fn" src/` | `functions where visibility = public` |
| Spring REST controllers | `grep -rn "@RestController" src/ \| grep "class "` | `classes where attributes contains '@RestController'` |
| Methods on UserService | `grep -rn "UserService" src/ \| grep "def \|public "` | `methods where parent = UserService` |
| Structs with 5+ fields | *(requires reading each file)* | `structs \| with fields \| where field_count > 5` |
| What does Config depend on | `grep -rn "Config" src/ \| grep "use \|import \|new "` | `symbol Config \| with deps` |

The grep column requires knowing the language's syntax, writing a correct regex, and filtering noise. The smartgrep column reads like a question.

### Sources

```
symbols / structs / classes / interfaces / records / functions / methods
traits / enums / impls / consts / types / modules
symbol <name>          -- single symbol lookup
deps [<name>]          -- dependencies (all, or for one symbol)
refs [<name>]          -- references (all, or to one symbol)
<source> in '<path>'   -- restrict to files matching a path substring
```

### Filters

```
where <field> <op> <value> [and|or ...]
```

| Field | Applies to |
|---|---|
| `name`, `file`, `visibility`, `kind`, `parent` | symbols |
| `attributes` | symbols with annotations/decorators |
| `field_count`, `param_count` | after `with fields` / `with params` |
| `from`, `to`, `dep_kind` | dependency rows |

| Operator | Meaning |
|---|---|
| `=` / `is` | equals (case-insensitive) |
| `!=` | not equals |
| `contains` | substring match |
| `starts_with` / `ends_with` | prefix / suffix |
| `>` `<` `>=` `<=` | numeric comparison |

### Pipeline stages

Stages follow `|` and apply left to right:

- `with fields` — add struct/class/enum field list and count
- `with params` — add function/method parameter list and count
- `with deps` — add outbound dependencies
- `with refs` — add inbound references
- `with signature` — add full type signature
- `show col1, col2` — select specific output columns
- `where field_count > 5` — filter after enrichment
- `sort field asc|desc` — sort results
- `limit N` — cap output

### Batch queries

Separate queries with `;` to run multiple in one call:

```bash
smartgrep query "structs | with fields; enums | with fields"
smartgrep query "classes where file contains 'service/'; methods where parent = UserController"
```

Results print under `# Query 1`, `# Query 2` headers.

### Path aliases

Long file paths in text output are automatically shortened. `src/main/java/com/example/catalog/controller/ProductController.java` becomes `[P]controller/ProductController.java`, with a `[paths]` legend at the top. JSON output always keeps full paths.

---

## Supported languages

- **Rust** — full support via tree-sitter-rust
- **Java** — full support via tree-sitter-java
- **Go** — full support via tree-sitter-go

Adding a language means writing one parser. The IR layer and query engine are language-agnostic.

---

## Architecture

```
Parser (tree-sitter) --> IR --> Index Builder --> Index --> Commands / Query Engine
```

- **Parser** (`src/parser/`) — language-specific, produces the IR
- **IR** (`src/ir/types.rs`) — language-agnostic symbol and dependency maps
- **Index** (`src/index/types.rs`) — queryable, with lookup tables for fast symbol resolution

Auto-indexing: queries trigger indexing implicitly. The index rebuilds when source files change.

See [AGENTS_README.md](AGENTS_README.md) for the agent-targeted reference.
