#!/usr/bin/env bash
# Repository policy check. Runs locally (pre-commit / commit-msg hooks) and in CI on every pull request.
#
#   check.sh files                 tracked files must not include assistant/tool configuration
#   check.sh commits <base> <head> commit messages must not carry tool attribution trailers
#   check.sh language <base> <head> added lines must be written in English
#   check.sh language --staged     same, for the staged diff (pre-commit)
#   check.sh message <file>        one commit message file (commit-msg hook)
#
# What it does NOT watch: text inside binary files, wording quality, or English that is merely poor.
# Language detection is a heuristic: a line fails when it holds two or more distinct words from
# scripts/policy/non-english-words.txt. Bank statement fixtures and translation catalogs are exempt
# because their content is real non-English data by design.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
WORDS="$HERE/non-english-words.txt"

# Assistant and editor-agent configuration. Contributors may use any tool they like; its
# configuration and attribution stay out of the history.
FORBIDDEN_PATHS='(^|/)(AGENTS\.md|CLAUDE(\.local)?\.md|GEMINI\.md|\.claude/|\.codex/|\.cursor/|\.cursorrules|\.windsurf/|\.windsurfrules|\.clinerules|\.aider[^/]*|\.continue/|\.github/copilot-instructions\.md|\.junie/)'

# Attribution trailers and footers added by AI coding tools (vendor-neutral list).
TRAILERS='^[[:space:]]*[A-Za-z-]+-Session:[[:space:]]*https?://|^[[:space:]]*Co-[Aa]uthored-[Bb]y:.*(noreply@anthropic\.com|claude|copilot|openai|codex|cursor|devin|gemini|deepseek|qwen|llama|aider|junie)|[Gg]enerated (with|by) .*(Claude|Copilot|Codex|Cursor|ChatGPT|Gemini|DeepSeek|Qwen|Aider|Junie)'

# Paths whose content is non-English data on purpose.
LANGUAGE_EXEMPT='^(spec/fixtures/|.*/locales/|.*/i18n/|scripts/policy/)'

fail() { echo "policy: $*" >&2; exit 1; }

check_files() {
  local hits
  hits=$(git ls-files | grep -E "$FORBIDDEN_PATHS" || true)
  [ -z "$hits" ] || fail "tool configuration files are not accepted in this repository:
$hits"
}

check_message_text() {
  if grep -Eiq "$TRAILERS"; then
    fail "commit message carries a tool attribution trailer; remove it (see CONTRIBUTING.md)."
  fi
}

check_commits() {
  local base=$1 head=$2 sha
  for sha in $(git rev-list "$base..$head"); do
    git log -1 --format=%B "$sha" | check_message_text || fail "commit $sha"
  done
}

added_lines() {
  # Prints "path<TAB>line" for every added line, skipping exempt paths.
  awk -v exempt="$LANGUAGE_EXEMPT" '
    /^\+\+\+ b\// { path = substr($0, 7); next }
    /^\+\+\+ /    { path = ""; next }
    /^\+/ && path != "" && path !~ exempt { print path "\t" substr($0, 2) }
  '
}

check_language() {
  local diff
  if [ "${1:-}" = "--staged" ]; then diff=$(git diff --cached -U0 --no-color); else diff=$(git diff -U0 --no-color "$1" "$2"); fi
  local bad
  bad=$(printf '%s\n' "$diff" | added_lines | python3 -c '
import re, sys
words = {w.strip().lower() for w in open(sys.argv[1], encoding="utf-8") if w.strip() and not w.startswith("#")}
for row in sys.stdin:
    path, _, line = row.rstrip("\n").partition("\t")
    found = {t for t in re.findall(r"[^\W\d_]+", line.lower()) if t in words}
    if len(found) >= 2:
        print(f"{path}: {line.strip()[:120]}")
' "$WORDS")
  [ -z "$bad" ] || fail "text must be written in English (code, comments, docs, commit messages):
$bad"
}

case "${1:-}" in
  files)    check_files ;;
  commits)  check_commits "$2" "$3" ;;
  language) shift; check_language "$@" ;;
  message)  check_message_text < "$2" ;;
  *)        fail "usage: check.sh files | commits <base> <head> | language <base> <head> | language --staged | message <file>" ;;
esac
