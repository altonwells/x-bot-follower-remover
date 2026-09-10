# Third-party references

This application was implemented independently. These projects informed the design and protocol investigation:

- [Ratatui 0.30.2](https://github.com/ratatui/ratatui): terminal initialization, table and modal patterns; MIT. Ratatui is a dependency.
- [X-Cleaner](https://github.com/taqui-786/X-Cleaner---Followers-Following): follower versus following semantics and native `RemoveFollower` request reference; MIT. No fork or vendored upstream extension is bundled.
- [Herdr](https://github.com/herdrdev/herdr): terminal restoration and help patterns; reference only.
- [n8n browser extension](https://github.com/n8n-io/n8n/tree/master/packages/%40n8n/mcp-browser-extension): connection generation and timeout patterns; reference only, no n8n code/dependency bundled.
- [X Bot Remover](https://github.com/vanrohan/xbotremover): profile/activity inspection research; reference only.

Rust and development dependency versions are recorded in Cargo.lock and extension/package-lock.json. Their respective license terms remain applicable. The packaged binary includes a machine-generated dependency license inventory in `DEPENDENCY_LICENSES.md` and installed Cargo package license files in `licenses/`.

The extension runtime contains application code bundled by esbuild; it has no third-party JavaScript runtime dependencies. Node/TypeScript/esbuild/Chrome type packages are development tools.
