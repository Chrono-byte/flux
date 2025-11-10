# Dotfiles Manager Roadmap

## Future Features

### LLM API Integration for Commit Messages

**Status:** Backlog

**Description:**
Integrate LLM API support to automatically generate commit messages based on file changes. This will reduce the need for manual commit message entry while maintaining meaningful commit history.

**Implementation Plan:**

1. **LLM Provider Support**
   - OpenAI API (via `async-openai` or `reqwest`)
   - Anthropic API (via `reqwest` or `anthropic-rs`)
   - Ollama local API (via `reqwest` to `http://localhost:11434`)
   - Custom endpoints (via `reqwest`)

2. **Configuration**
   - Add `[llm]` section to `config.toml`:
     ```toml
     [llm]
     provider = "openai"  # openai, anthropic, ollama, custom
     api_key = "env:OPENAI_API_KEY"  # or direct key, or env:VAR_NAME
     model = "gpt-4o-mini"
     base_url = ""  # optional for custom endpoints (e.g., Ollama)
     ```

3. **Implementation Files**
   - `src/llm.rs` - LLM integration module
     - `generate_commit_message(changes: Vec<FileChange>) -> Result<String>`
     - `check_llm_availability() -> Result<bool>`
     - Provider-specific implementations

4. **Error Handling**
   - Network errors: Fallback to user prompts
   - API errors: Fallback to user prompts
   - Timeout: Fallback to user prompts
   - Invalid API key: Fallback to user prompts

5. **Prompt Template**
   ```
   Generate a concise git commit message (max 72 chars) for these dotfile changes:
   [list of changes with file paths and change types]
   ```

6. **Integration Points**
   - Modify `src/git.rs::commit_changes()` to use LLM if available
   - Modify `src/main.rs` sync command to check LLM availability
   - Fallback to existing `prompt_commit_message()` if LLM unavailable

7. **Dependencies to Add**
   - `reqwest` with `tokio` (async HTTP for LLM APIs)
   - `async-openai` (optional, for OpenAI-specific features)
   - `tokio` runtime (for async operations)

**Testing Considerations:**
- Test with each provider
- Test fallback behavior when API is unavailable
- Test rate limiting and error handling
- Test with various change types (additions, modifications, deletions)

**Future Enhancements:**
- Cache API responses for similar changes
- Support for commit message templates
- Batch processing for multiple changes
- Cost tracking for API usage

