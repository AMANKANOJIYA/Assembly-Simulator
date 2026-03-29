# Maintainer checklist (GitHub settings)

These options live on GitHub and cannot be set from the repository files. Use this as a one-time setup for **open collaboration** and **security**.

## Collaboration

| Setting | Where | Suggestion |
|--------|--------|-------------|
| **Issues** | Repository → **Settings** → **General** → **Features** | Keep **Issues** enabled (default). |
| **Discussions** | Same | Optional: enable **Discussions** for Q&A and ideas; keep **Issues** for bugs and tracked work. |
| **Pull requests** | **Settings** → **General** | Allow **merge**, **squash**, and/or **rebase** as you prefer. Many teams use **squash merge** for a linear history. |
| **Default branch** | **Settings** → **Branches** | Usually `main`; protect it if you add CI or required reviews. |

## Security

| Setting | Where | Suggestion |
|--------|--------|-------------|
| **Private vulnerability reporting** | **Settings** → **Security** → **Code security** | Enable **Private vulnerability reporting** so researchers can report without a public issue (pairs with [SECURITY.md](../SECURITY.md)). |

## Discovery

| Setting | Where | Suggestion |
|--------|--------|-------------|
| **About** | Repository home → **⚙️** next to *About* | Add description, **Topics** (e.g. `tauri`, `rust`, `react`, `education`), optional website. |
| **License** | Detected from `LICENSE` | Confirm GitHub shows **MIT** on the repo page. |

## Optional automation

- **Branch protection** on `main`: require PRs, status checks, or reviews when you add CI.
- **GitHub Actions** for `npm run build`, `npm run lint`, and `cargo check` on pull requests.
