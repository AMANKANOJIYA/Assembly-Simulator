# Security

## Supported versions

Security updates are applied to the **default branch** (e.g. `main`) and included in the next release. Use the latest tagged release or commit when possible.

## Reporting a vulnerability

**Please do not** file a public GitHub issue for undisclosed security vulnerabilities.

Instead:

1. **GitHub (preferred when enabled)** — On the repository page, open the **Security** tab and use **Report a vulnerability**. Maintainers should enable **private vulnerability reporting** under **Settings → Security → Code security** so reports stay non-public. See [GitHub: Privately reporting a security vulnerability](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing/privately-reporting-a-security-vulnerability).
2. **Maintainer contact** — If GitHub reporting is unavailable, contact the maintainers through a channel they publish on their GitHub profile or org (do not post exploit details in public issues).

Include:

- Description of the issue and potential impact
- Steps to reproduce (if applicable)
- Affected component (frontend, Tauri/Rust backend, plugin, etc.)
- Your assessment of severity (if you have one)

We will aim to acknowledge receipt and coordinate next steps. Response times depend on maintainer availability (this is often a volunteer project).

## Scope

This policy applies to the **Assembly Simulator** application and its **first-party code** in this repository. Third-party dependencies (Rust crates, npm packages) should be reported to their upstream projects when appropriate.

## Safe harbor

We appreciate responsible disclosure and will not pursue legal action against researchers who follow this process and act in good faith.
