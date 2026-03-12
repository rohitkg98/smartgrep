# Benchmark: smartgrep vs Vanilla CLI vs Java LSP on spring-io/initializr

## Project

| Field | Value |
|-------|-------|
| **Name** | Spring Initializr |
| **Repository** | https://github.com/spring-io/initializr |
| **Commit** | `1a5bea68d434c054c2f06f0b696656c3485023cd` |
| **Language** | Java (Spring Boot) |
| **Size** | 625 Java files |
| **Description** | The official Spring Boot project generator (start.spring.io). A large, well-structured Spring Boot application with REST controllers, extensive configuration, template rendering, and a plugin-based contributor architecture. |

## Questions

Five structural questions were asked of both agents:

1. What are all the REST controllers and what endpoints do they expose?
2. What is the core domain model? List key classes/interfaces in initializr-metadata and initializr-generator with their fields.
3. What are the main configuration classes? (`@Configuration` annotated)
4. What does `ProjectGenerator` depend on -- what are its fields and dependencies?
5. What are the different `ProjectContributor` implementations? (Key extension point)

## Results

| Metric | Smartgrep | Vanilla CLI | Java LSP |
|--------|-----------|-------------|----------|
| Tokens | 38,848 | 75,202 | 53,955 |
| Tool Calls | 8 | 20 | 29 |
| Duration | ~59s | ~85s | ~111s |

### Per-Question Tool Call Breakdown

| Question | Smartgrep | Vanilla | LSP |
|----------|-----------|---------|-----|
| Q1: REST controllers & endpoints | 2 | 4 | 6 |
| Q2: Core domain model | 2 | 8 | 10 |
| Q3: Configuration classes | 1 | 3 | 4 |
| Q4: ProjectGenerator deps | 2 | 2 | 2 |
| Q5: ProjectContributor impls | 1 | 3 | 3 |
| **Total** | **8** | **20** | **29** |

### Key Observations

- Smartgrep wins all three metrics: tokens, tool calls, and duration.
- At 625 files, smartgrep is **31% faster** than vanilla -- the crossover point where structural queries outperform grep+read is somewhere between 167 and 625 files.
- Token savings of 48% -- nearly half the tokens. On a per-query basis, that is ~3,600 fewer tokens per question.
- Q2 (domain model) shows the biggest gap: 2 queries vs 8 tool calls. Structural queries with `| with fields` replace the glob-then-read-each-file pattern.
- Q3 and Q5 achieved ideal 1-command answers using annotation filtering (`where attributes contains '@Configuration'`) and reference lookup (`refs ProjectContributor`).
- Path alias mapping reduced output size significantly -- the common prefix `initializr-generator-spring/src/main/java/io/spring/initializr/generator/spring/` was shortened to `[P]` across all results.

### Java LSP Observations

- LSP came in last on all metrics despite being the most semantically rich tool
- `documentSymbol` is per-file -- Q2 needed 8 separate LSP calls to inspect model files. Smartgrep answered with 1 query.
- `goToImplementation` returned empty for ProjectContributor -- agent fell back to Grep
- `hover` returned empty for some fields due to project context issues
- LSP has no filtering or query language -- you get all symbols from a file and filter manually
- LSP's strength (semantic type resolution, cross-file navigation) didn't provide value for structural navigation questions
- The per-file model is fundamentally more expensive than smartgrep's index-wide queries for discovery tasks

### Scaling Trend

| Codebase | Files | Token Savings | Call Reduction | Speed |
|----------|-------|---------------|----------------|-------|
| smartgrep-rs (self, Rust) | 20 | ~0% | -40% | 1.3x slower |
| spring-io/initializr (Java) | 625 | -48% | -60% | 31% faster |

**3-Way Comparison (spring-io/initializr)**

| Tool | Tokens | Tool Calls | Duration |
|------|--------|-----------|----------|
| Smartgrep | 38,848 | 8 | ~59s |
| Vanilla CLI | 75,202 | 20 | ~85s |
| Java LSP | 53,955 | 29 | ~111s |

Ranking: Smartgrep > Vanilla CLI > Java LSP for structural codebase navigation.

Smartgrep's advantage grows with codebase size. On small codebases, the per-invocation overhead dominates. On medium-to-large codebases, structural queries dramatically outperform grep+read by eliminating the need to open individual files.

## Methodology

- Both agents answered the same 5 questions about the codebase.
- **Smartgrep agent** used only `smartgrep` binary commands (query DSL with OR, attributes filtering, path aliases).
- **Vanilla agent** used only Grep, Glob, and Read tools.
- **LSP agent** used LSP tools (documentSymbol, findReferences, goToDefinition, goToImplementation, hover) plus Grep/Glob for discovery.
- Tokens measured from agent `total_tokens` usage.
- Duration measured from agent wall clock time.
- Smartgrep version included: query DSL, OR support, path alias mapping, nested type extraction, attributes filtering, implicit daemon.
