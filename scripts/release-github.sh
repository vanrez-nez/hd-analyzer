#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

usage() {
  cat <<'EOF'
Usage:
  scripts/release-github.sh <major|minor|patch>
  scripts/release-github.sh --resume <version>

Bumps all app versions, validates the workspace, builds the macOS DMG,
commits/tags/pushes the release, and creates a draft GitHub release.

Use --resume when a previous release run already bumped versions and built
the DMG but stopped before committing, tagging, pushing, or creating the
GitHub release.
EOF
}

die() {
  echo "error: $*" >&2
  exit 1
}

run() {
  echo "+ $*"
  "$@"
}

tmp_dir="$(mktemp -d)"
trap 'rm -rf "$tmp_dir"' EXIT

release_files=(
  Cargo.toml
  Cargo.lock
  src-tauri/tauri.conf.json
  src-web/package.json
  src-web/package-lock.json
)

ensure_gh_auth() {
  if gh auth status >/dev/null 2>&1; then
    return
  fi

  echo "GitHub CLI is not authenticated. Starting: gh auth login -h github.com"
  run gh auth login -h github.com
  run gh auth status >/dev/null
}

ensure_clean() {
  [[ -z "$(git status --porcelain)" ]] || die "working tree must be clean before releasing"
}

ensure_resume_dirty_files_are_expected() {
  local allowed=("${release_files[@]}" "scripts/release-github.sh")
  local unexpected=()
  local line path

  while IFS= read -r line; do
    [[ -n "$line" ]] || continue
    path="${line:3}"
    case " ${allowed[*]} " in
      *" ${path} "*) ;;
      *) unexpected+=("$path") ;;
    esac
  done < <(git status --porcelain)

  if (( ${#unexpected[@]} > 0 )); then
    printf 'error: --resume only allows release files to be dirty. Unexpected paths:\n' >&2
    printf '  %s\n' "${unexpected[@]}" >&2
    exit 1
  fi
}

find_dmg() {
  local version="$1"
  for dir in target src-tauri/target; do
    [[ -d "$dir" ]] || continue
    find "$dir" -path "*/bundle/dmg/*${version}*.dmg" -type f -print
  done | head -n 1
}

mode="bump"
bump="${1:-}"
resume_version=""

case "$bump" in
  major|minor|patch) ;;
  --resume|resume)
    mode="resume"
    resume_version="${2:-}"
    [[ "$resume_version" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]] \
      || die "--resume requires a plain SemVer version, for example: scripts/release-github.sh --resume 0.2.0"
    ;;
  -h|--help|"")
    usage
    exit 0
    ;;
  *)
    usage >&2
    die "release bump must be one of: major, minor, patch"
    ;;
esac

[[ "$(uname -s)" == "Darwin" ]] || die "GitHub DMG releases must be built on macOS"

for cmd in cargo gh git node npm; do
  command -v "$cmd" >/dev/null 2>&1 || die "missing required command: $cmd"
done

[[ -d src-web/node_modules ]] || die "missing src-web/node_modules. Run: npm --prefix src-web install"

branch="$(git branch --show-current)"
[[ -n "$branch" ]] || die "must be on a branch, not detached HEAD"
upstream="$(git rev-parse --abbrev-ref --symbolic-full-name '@{u}' 2>/dev/null)" \
  || die "current branch must have an upstream remote"

if [[ "$mode" == "resume" ]]; then
  ensure_resume_dirty_files_are_expected
else
  ensure_clean
fi

ensure_gh_auth

cat > "$tmp_dir/read-versions.js" <<'NODE'
const fs = require("fs");

function readJson(path) {
  return JSON.parse(fs.readFileSync(path, "utf8"));
}

function workspaceVersion() {
  const text = fs.readFileSync("Cargo.toml", "utf8");
  const match = text.match(/\[workspace\.package\][\s\S]*?\nversion\s*=\s*"([^"]+)"/);
  if (!match) throw new Error("Cargo.toml is missing [workspace.package] version");
  return match[1];
}

function lockVersions() {
  const text = fs.readFileSync("Cargo.lock", "utf8");
  const versions = {};
  for (const block of text.split(/\n(?=\[\[package\]\])/)) {
    const name = block.match(/^name = "([^"]+)"/m)?.[1];
    const version = block.match(/^version = "([^"]+)"/m)?.[1];
    if (name === "space-lenser-core" || name === "space-lenser-tauri") {
      versions[name] = version;
    }
  }
  return versions;
}

const versions = {
  workspace: workspaceVersion(),
  tauri: readJson("src-tauri/tauri.conf.json").version,
  packageJson: readJson("src-web/package.json").version,
  packageLockRoot: readJson("src-web/package-lock.json").version,
  packageLockPackage: readJson("src-web/package-lock.json").packages?.[""]?.version,
  ...lockVersions(),
};

console.log(JSON.stringify(versions));
NODE

cat > "$tmp_dir/current-version.js" <<'NODE'
const versions = JSON.parse(process.env.VERSIONS_JSON);
const entries = Object.entries(versions);
const expected = versions.workspace;
const mismatches = entries.filter(([, version]) => version !== expected);
if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(expected)) {
  console.error(`current workspace version is not plain SemVer: ${expected}`);
  process.exit(1);
}
if (mismatches.length) {
  console.error(`version files are not in sync with workspace version ${expected}:`);
  for (const [name, version] of mismatches) {
    console.error(`  ${name}: ${version ?? "<missing>"}`);
  }
  process.exit(1);
}
console.log(expected);
NODE

cat > "$tmp_dir/next-version.js" <<'NODE'
const [major, minor, patch] = process.env.CURRENT_VERSION.split(".").map(Number);
switch (process.env.BUMP) {
  case "major":
    console.log(`${major + 1}.0.0`);
    break;
  case "minor":
    console.log(`${major}.${minor + 1}.0`);
    break;
  case "patch":
    console.log(`${major}.${minor}.${patch + 1}`);
    break;
  default:
    process.exit(1);
}
NODE

versions_json="$(node "$tmp_dir/read-versions.js")"
current_version="$(VERSIONS_JSON="$versions_json" node "$tmp_dir/current-version.js")"

if [[ "$mode" == "resume" ]]; then
  [[ "$current_version" == "$resume_version" ]] \
    || die "--resume ${resume_version} does not match synchronized version files at ${current_version}"
  next_version="$resume_version"
else
  next_version="$(CURRENT_VERSION="$current_version" BUMP="$bump" node "$tmp_dir/next-version.js")"
fi

tag="v${next_version}"

git rev-parse "$tag" >/dev/null 2>&1 && die "local tag already exists: $tag"
git ls-remote --exit-code --tags origin "refs/tags/${tag}" >/dev/null 2>&1 && die "remote tag already exists: $tag"

echo "Preparing release ${tag} on ${branch} (${upstream})"

cat > "$tmp_dir/update-versions.js" <<'NODE'
const fs = require("fs");

const current = process.env.CURRENT_VERSION;
const next = process.env.NEXT_VERSION;

function write(path, text) {
  fs.writeFileSync(path, text);
}

function updateJson(path, mutator) {
  const json = JSON.parse(fs.readFileSync(path, "utf8"));
  mutator(json);
  write(path, `${JSON.stringify(json, null, 2)}\n`);
}

function replaceExact(path, pattern, replacement) {
  const text = fs.readFileSync(path, "utf8");
  const nextText = text.replace(pattern, replacement);
  if (nextText === text) throw new Error(`no version replacement made in ${path}`);
  write(path, nextText);
}

replaceExact(
  "Cargo.toml",
  new RegExp(`(\\[workspace\\.package\\][\\s\\S]*?\\nversion\\s*=\\s*)"${current}"`),
  `$1"${next}"`
);

updateJson("src-tauri/tauri.conf.json", (json) => {
  if (json.version !== current) throw new Error("src-tauri/tauri.conf.json version changed during release");
  json.version = next;
});

updateJson("src-web/package.json", (json) => {
  if (json.version !== current) throw new Error("src-web/package.json version changed during release");
  json.version = next;
});

updateJson("src-web/package-lock.json", (json) => {
  if (json.version !== current) throw new Error("src-web/package-lock.json root version changed during release");
  if (!json.packages?.[""] || json.packages[""].version !== current) {
    throw new Error("src-web/package-lock.json package version changed during release");
  }
  json.version = next;
  json.packages[""].version = next;
});

const lockPath = "Cargo.lock";
const lockText = fs.readFileSync(lockPath, "utf8");
const lockNext = lockText
  .split(/\n(?=\[\[package\]\])/)
  .map((block) => {
    const name = block.match(/^name = "([^"]+)"/m)?.[1];
    if (name !== "space-lenser-core" && name !== "space-lenser-tauri") {
      return block;
    }
    const replaced = block.replace(new RegExp(`^version = "${current}"$`, "m"), `version = "${next}"`);
    if (replaced === block) throw new Error(`Cargo.lock ${name} version was not ${current}`);
    return replaced;
  })
  .join("\n");
write(lockPath, lockNext);
NODE

if [[ "$mode" == "bump" ]]; then
  CURRENT_VERSION="$current_version" NEXT_VERSION="$next_version" node "$tmp_dir/update-versions.js"

  run cargo fmt --check
  run cargo test
  run npm --prefix src-web run typecheck
  run scripts/build-dmg.sh
fi

dmg="$(find_dmg "$next_version")"
if [[ -z "$dmg" ]]; then
  run scripts/build-dmg.sh
  dmg="$(find_dmg "$next_version")"
fi
[[ -n "$dmg" ]] || die "could not find DMG for version ${next_version}"

run git add "${release_files[@]}"
if ! git diff --quiet -- scripts/release-github.sh; then
  run git add scripts/release-github.sh
fi
run git commit -m "chore: release ${tag}"
run git tag -a "$tag" -m "Release ${tag}"
run git push origin "$branch"
run git push origin "$tag"
run gh release create "$tag" "$dmg" --draft --generate-notes --title "Space Lenser ${tag}"

echo "Draft GitHub release created for ${tag}"
echo "Uploaded artifact: ${dmg}"
