# How to Identify Real Unused Code vs False Positives

## The Problem
Rust's dead code warnings can be misleading because they're based on **visibility** and **reachability from the public API**, not actual usage.

## Key Insights

### 1. Module Visibility Matters
When modules aren't publicly exposed in `lib.rs`, all their contents appear "unused" to the compiler even if they're used internally:

```rust
// lib.rs
mod services;  // PRIVATE - everything inside appears unused
pub mod services;  // PUBLIC - compiler sees external usage
```

**Fix Applied**: Made modules public in `lib.rs` → Reduced warnings from 414 to 234!

### 2. Trait Methods Can Be Genuinely Unused
Even if a trait is used, individual methods might never be called:

```rust
trait MediaBackend {
    async fn get_libraries(&self) -> Result<Vec<Library>>;  // Used ✓
    async fn mark_watched(&self) -> Result<()>;  // Never called ✗
}
```

**How to Check**:
```bash
# Find all calls to a trait method
grep -r "\.method_name(" src --include="*.rs" | \
  grep -v "impl TraitName" | \
  grep -v "trait TraitName"
```

### 3. JSON DTOs Are False Positives
Structs used for deserialization appear unused because they're populated by serde:

```rust
#[derive(Deserialize)]
struct ApiResponse {  // Appears unused but needed for JSON parsing
    field: String,
}
```

**Identification**: Look for `#[derive(Deserialize)]` or `#[derive(Serialize)]`

### 4. Dynamic Dispatch Hides Usage
Code used through trait objects appears unused:

```rust
let backend: Arc<dyn MediaBackend> = ...;  // Concrete type usage hidden
backend.some_method();  // Compiler can't track back to implementation
```

## Systematic Approach to Find Real Unused Code

### Step 1: Fix Visibility First
```bash
# Check if modules are exposed
grep "^mod " src/lib.rs  # Should be "pub mod" for library crates
```

### Step 2: Analyze Trait Methods
```bash
# List all trait methods
grep "async fn\|fn " src/path/to/trait.rs

# Check each method's usage
for method in method_list; do
    echo "$method:"
    grep -r "\.$method(" src --include="*.rs" | \
      grep -v "impl " | wc -l
done
```

### Step 3: Check Function/Method Calls
```bash
# Find all function definitions
grep -r "^pub fn\|^pub async fn" src --include="*.rs"

# Check if they're called
grep -r "function_name(" src --include="*.rs"
```

### Step 4: Identify Safe-to-Remove Patterns
- Methods with 0 calls outside their trait definition
- Structs only used in never-called methods
- Functions not exported and not called internally
- Test-only code in non-test modules

### Step 5: Use Tools
```bash
# Use cargo-udeps for dependency checking
cargo install cargo-udeps
cargo +nightly udeps

# Use cargo-machete for unused dependencies
cargo install cargo-machete
cargo machete
```

## Categories of Warnings

### Genuinely Unused (Safe to Remove)
- Never-called trait methods
- Orphaned helper functions
- Abandoned feature code
- Unused struct fields (if not for serde)

### False Positives (Keep with #[allow(dead_code)])
- JSON DTOs with serde
- FFI structs
- Code awaiting implementation
- Public API for future features

### Misleading (Fix Visibility)
- Private modules with public intent
- Internal APIs used by tests
- Cross-crate boundaries

## Applied Example: MediaBackend Trait

Found unused methods by:
1. Listed all trait methods
2. Searched for actual calls: `grep -r "\.method(" src`
3. Found these were never called:
   - mark_watched
   - mark_unwatched
   - get_watch_status
   - search
   - fetch_episode_markers
   - etc.

Result: Removed 12+ unused trait methods and their implementations!

## Best Practices

1. **Make modules public** if they contain code used by tests or other crates
2. **Remove unused trait methods** - they add maintenance burden
3. **Document why** code appears unused with comments
4. **Use #[cfg(test)]** for test-only code
5. **Regular cleanup** - unused code accumulates over time

## Quick Commands

```bash
# Count current warnings
cargo check 2>&1 | grep -c "warning:"

# Find unused functions in a module
grep -r "fn " src/module --include="*.rs" | \
  while read line; do
    func=$(echo $line | sed 's/.*fn \([a-z_]*\).*/\1/')
    count=$(grep -r "$func(" src --include="*.rs" | wc -l)
    [ $count -eq 1 ] && echo "Possibly unused: $func"
  done

# Find never-constructed structs
grep "^pub struct" src/**/*.rs | \
  while read line; do
    struct=$(echo $line | awk '{print $3}')
    grep -r "$struct::\|$struct {" src --include="*.rs" || \
      echo "Never constructed: $struct"
  done
```