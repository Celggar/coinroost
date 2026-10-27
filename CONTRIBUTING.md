# Contributing to Coinroost

Thanks for helping. A few rules keep the project consistent and the history clean.

## How changes land

1. Fork the repository and create a branch from `main`.
2. Open a pull request. `main` is protected: nothing is pushed to it directly, and every pull request needs the
   maintainer's review before it is merged.
3. CI must be green. Workflows from first-time and external contributors start after the maintainer approves them.

Small, focused pull requests are reviewed faster than large ones. For anything bigger than a bug fix, open an issue
first so the approach can be agreed before you write the code.

## Language

Everything in the repository is written in **English**: code, identifiers, comments, documentation, commit
messages, issues and pull requests. The only exceptions are translation catalogs and bank statement fixtures, whose
content is real data. The policy check rejects added lines that read as another language.

## Commits

- [Conventional Commits](https://www.conventionalcommits.org/): `feat(normalizer): parse XLSX statements`.
- Imperative mood, subject of 50 characters or fewer.
- `Co-Authored-By` is welcome for people who worked on the change.

## Tools

Use whatever editor, assistant or tool you like. What lands in the repository is your work, so:

- no tool attribution trailers or footers in commit messages;
- no assistant or agent configuration files (for example `AGENTS.md` or editor rule folders). Keep them in your
  global git ignore or in `.git/info/exclude`.

The policy check (`scripts/policy/check.sh`) enforces both, locally and in CI. To run it before every commit:

```sh
git config core.hooksPath .githooks
```

## Tests and data

- Bank statement fixtures must be **synthetic**. Never commit a real statement, account number, name or balance,
  even redacted.
- A test should catch a real bug: say which input breaks it.

## License

By contributing you agree that your contribution is licensed under the [MIT License](LICENSE) of this repository.
