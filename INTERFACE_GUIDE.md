# M Engine: Dual Interface Architecture

## Quick Answer to Your Questions

### 1. **Are errors displayed properly in the interface?** ✅ YES
- Web interface shows **detailed error messages** in the "Errors" tab
- Errors include line/column info when available
- Click the **Debug button** to see AST and tokens for troubleshooting
- Color highlights: 🔴 Errors, 🟢 Success, 🔵 Info

### 2. **Can you still run and view tokens, AST in VS Code console?** ✅ YES
- Use `cargo run --bin cli --debug` to see everything in console
- `println!` statements now go to **terminal output**
- You have **both**: Web UI for users + CLI for debugging

---

## Architecture

```
┌─ WEB INTERFACE (Production/Demo) ─────────────────────┐
│  http://localhost:8080                                │
│  • Monaco Editor                                       │
│  • Real-time compilation with Ctrl+Enter               │
│  • Output, Errors, SQL, Formatted Code tabs            │
│  • Debug panel (AST + Tokens on demand)                │
└─ cargo run --bin web ────────────────────────────────┘
         ↓
    src/api.rs (HTTP handlers)
         ↓
    src/lib.rs (compile_formula function)
         ↑
┌─────────┴────────────────────────────────────────────┐
│  SHARED COMPILER ENGINE (pq_lexer → pq_parser → ... ) │
└──────────────────────────────────────────────────────┘
         ↑
┌─ CLI DEBUG MODE (Development) ────────────────────────┐
│  Terminal/Console Output                              │
│  ✓ View tokens (println in terminal)                  │
│  ✓ View AST (`{:#?}` formatting)                      │
│  ✓ Error diagnostics with spans                       │
│  ✓ All internal compiler state                        │
└─ cargo run --bin cli 'formula' --debug ──────────────┘
```

---

## Running Both Modes

### **Web Interface (For Users)**
```bash
cargo run --bin web
# Then open: http://localhost:8080
```

**Features:**
- 📝 Monaco code editor
- 🔘 Compile button or Ctrl+Enter
- 📊 Results in tabbed panels
- 🐛 Debug button toggles AST/token view
- ✨ Formatted code display
- 📈 SQL generation display
- 🎨 Dark theme

### **CLI Debug Mode (For Developers)**
```bash
# Simple formula
cargo run --bin cli 'let x = 1 in x' --debug

# From file
cargo run --bin cli -f formula.pq --debug

# With data
cargo run --bin cli 'Table.SelectRows(t, each [Age]>25)' --data '{"rows":[[30]]}'
```

**Output:** Console shows everything:
```
🔧 M Engine CLI
============================================================
📋 Formula:
let x = 1 in x
...
✓ Compilation successful!
============================================================
📝 Formatted Code:
let x = 1 in x
============================================================
🌳 AST:
Program {
  steps: [...],
  final_expr: ...
}
============================================================
🔤 Tokens:
  Token { kind: ... }
  ...
```

---

## File Structure

```
M_Engine/
├── Cargo.toml                    # Dependencies + binary configs
├── src/
│   ├── lib.rs                    # Shared: CompileRequest, compile_formula()
│   ├── api.rs                    # HTTP handlers: /api/compile, /api/compile/debug
│   ├── main.rs                   # Shows usage instructions
│   └── bin/
│       ├── web.rs               # Web server (Actix-web + Monaco)
│       └── cli.rs               # CLI tool with debug output
├── public/
│   └── index.html               # Playground UI
└── crates/
    └── [compiler crates]
```

---

## Error Handling

### Web Interface Errors
```json
{
  "success": false,
  "errors": [
    {
      "message": "unknown step reference: 'MyStep'",
      "line": 3,
      "column": 10,
      "context": null
    }
  ],
  "result": null,
  "formatted_code": null,
  "sql": null
}
```
→ Displayed in red box with line numbers

### CLI Errors
```
Error: unknown step reference: 'MyStep'
  at line 3
```
→ Prints to stderr in terminal

---

## Testing

### Run All Tests
```bash
cargo test
```
Output shows in console:
```
running 5 tests
test tests::test_compile_simple ... ok
test tests::test_ast_structure ... ok
...
```

### Run Specific Test
```bash
cargo test test_compile_simple -- --nocapture
```
(--nocapture shows println! output)

---

## Key Improvements Made

| Aspect | Before | After |
|--------|--------|-------|
| **Error Display** | Simple string | Structured with line/column |
| **Debug Info** | Nowhere | Web UI + CLI both support |
| **Interfaces**  | None | Web (prod) + CLI (dev) |
| **Tokens/AST** | Console only | Both web (optional) + CLI |
| **Testing** | Manual | `cargo test` works perfectly |
| **Production** | N/A | Ready with web server |
| **Development** | N/A | CLI for debugging |

---

## Next Steps

### 1. **Test the Web Server**
```bash
cargo run --bin web
# Open http://localhost:8080
```

### 2. **Test CLI Debug Mode**
```bash
cargo run --bin cli "let x = [1,2,3] in x" --debug
```

### 3. **Add More Features** (Optional)
- Real-time error checking (WebSocket)
- Code completion in editor
- Save/export results
- Shared playground links
- Theme switcher

---

## FAQ

**Q: Will the web interface slow down the compiler?**  
A: No. The web server just serializes responses. Compilation happens identically.

**Q: Can I use both simultaneously?**  
A: Yes! Run `cargo run --bin web` in one terminal and `cargo run --bin cli` in another.

**Q: Where do println! statements go?**  
A: They go to the terminal/console (stdout). The web interface shows structured errors/output instead.

**Q: Can users see the tokens/AST online?**  
A: Only if they click the **Debug button** - then it makes a request to `/api/compile/debug`.

**Q: How do I deploy the web version?**  
A: Build with `cargo build --release --bin web`, then run the binary. It listens on port 8080.
