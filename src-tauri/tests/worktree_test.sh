#!/usr/bin/env bash
# ===========================================================================
# Worktree module integration tests
# Tests the core git worktree lifecycle without Tauri dependencies
# ===========================================================================
set -euo pipefail

PASS=0
FAIL=0
ERRORS=""

pass() { PASS=$((PASS+1)); echo "  [PASS] $1"; }
fail() { FAIL=$((FAIL+1)); ERRORS="${ERRORS}\n  [FAIL] $1"; echo "  [FAIL] $1"; }

# ── Setup ──────────────────────────────────────────────────────────────────
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

PROJECT="$TMPDIR/project"
mkdir -p "$PROJECT"
cd "$PROJECT"

git init -b main . >/dev/null 2>&1
git config user.email "test@test.com"
git config user.name "Test"
git config commit.gpgsign false
echo "# Test" > README.md
echo '{}' > package.json
echo "node_modules/" > .gitignore
git add README.md package.json .gitignore
git commit -m "Initial commit" >/dev/null 2>&1

# Create node_modules AFTER initial commit (untracked, ignored)
mkdir -p node_modules/some-pkg
echo '{}' > node_modules/some-pkg/package.json

WORKTREE_BASE=".scrum-ai-worktrees"
TASK_ID="test-001"
WT_PATH="${WORKTREE_BASE}/task-${TASK_ID}"
BRANCH="feature/task-${TASK_ID}"

echo ""
echo "========================================="
echo " Worktree Integration Tests"
echo "========================================="
echo ""

# ── Test 1: Create worktree ────────────────────────────────────────────────
echo "Test 1: Worktree creation"
mkdir -p "$WORKTREE_BASE"
git worktree add "$WT_PATH" -b "$BRANCH" main >/dev/null 2>&1

if [ -d "$WT_PATH" ] && [ -f "$WT_PATH/README.md" ]; then
    pass "Worktree directory created with files from main"
else
    fail "Worktree directory or files missing"
fi

if git branch | grep -q "$BRANCH"; then
    pass "Feature branch created"
else
    fail "Feature branch not found"
fi

# ── Test 2: .gitignore entry ──────────────────────────────────────────────
echo ""
echo "Test 2: .gitignore management"

# Simulate ensure_gitignore_entry
ENTRY=".scrum-ai-worktrees/"
if ! grep -qx "$ENTRY" .gitignore 2>/dev/null; then
    echo "$ENTRY" >> .gitignore
fi
if grep -qx "$ENTRY" .gitignore; then
    pass ".gitignore contains worktree entry"
else
    fail ".gitignore missing worktree entry"
fi

# Idempotency check: simulate calling ensure_gitignore_entry again
# (it should NOT add a duplicate since entry already exists)
if ! grep -qx "$ENTRY" .gitignore 2>/dev/null; then
    echo "$ENTRY" >> .gitignore
fi
COUNT=$(grep -cx "$ENTRY" .gitignore)
if [ "$COUNT" -eq 1 ]; then
    pass ".gitignore entry is idempotent (1 occurrence)"
else
    fail ".gitignore has $COUNT occurrences instead of 1"
fi

# ── Test 3: node_modules symlink ──────────────────────────────────────────
echo ""
echo "Test 3: node_modules symlink"

ln -s "$PROJECT/node_modules" "$WT_PATH/node_modules" 2>/dev/null || true

if [ -L "$WT_PATH/node_modules" ]; then
    pass "node_modules symlink created"
else
    fail "node_modules symlink missing"
fi

if [ -f "$WT_PATH/node_modules/some-pkg/package.json" ]; then
    pass "Symlinked node_modules accessible"
else
    fail "Cannot access files through symlink"
fi

# ── Test 4: Work in worktree and auto-commit ──────────────────────────────
echo ""
echo "Test 4: Auto-commit in worktree"

echo "new content" > "$WT_PATH/feature.txt"
cd "$WT_PATH"
git add -A >/dev/null 2>&1
git commit -m "[MicroScrum AI] 自動コミット: エージェント作業完了" >/dev/null 2>&1
cd "$PROJECT"

LAST_MSG=$(cd "$WT_PATH" && git log --oneline -1)
if echo "$LAST_MSG" | grep -q "自動コミット"; then
    pass "Auto-commit created with correct message"
else
    fail "Auto-commit message incorrect: $LAST_MSG"
fi

# ── Test 5: Diff between main and feature branch ─────────────────────────
echo ""
echo "Test 5: Diff retrieval"

DIFF_FILES=$(git diff --name-only "main...$BRANCH")
if echo "$DIFF_FILES" | grep -q "feature.txt"; then
    pass "Diff correctly shows changed files"
else
    fail "Diff doesn't show feature.txt: $DIFF_FILES"
fi

DIFF_STAT=$(git diff --stat "main...$BRANCH")
if echo "$DIFF_STAT" | grep -q "feature.txt"; then
    pass "Diff stat shows feature.txt"
else
    fail "Diff stat missing feature.txt"
fi

# ── Test 6: Merge success ─────────────────────────────────────────────────
echo ""
echo "Test 6: Successful merge into main"

# Commit any changes on main (like .gitignore) to avoid dirty state
git add -A >/dev/null 2>&1
git diff --cached --quiet || git commit -m "Update gitignore" >/dev/null 2>&1

# Remove symlink before merge cleanup
rm -f "$WT_PATH/node_modules"

git merge --no-ff -m "[MicroScrum AI] Merge task-${TASK_ID}" "$BRANCH" >/dev/null 2>&1
MERGE_OK=$?

if [ $MERGE_OK -eq 0 ]; then
    pass "Merge completed successfully"
else
    fail "Merge failed with exit code $MERGE_OK"
fi

if [ -f "$PROJECT/feature.txt" ]; then
    pass "Merged file exists on main"
else
    fail "Merged file missing on main"
fi

# Clean up worktree and branch
git worktree remove "$WT_PATH" --force >/dev/null 2>&1
git worktree prune >/dev/null 2>&1
git branch -d "$BRANCH" >/dev/null 2>&1

if [ ! -d "$WT_PATH" ]; then
    pass "Worktree removed after merge"
else
    fail "Worktree still exists after cleanup"
fi

if ! git branch | grep -q "$BRANCH"; then
    pass "Branch deleted after merge"
else
    fail "Branch still exists after cleanup"
fi

# ── Test 7: Merge conflict detection ─────────────────────────────────────
echo ""
echo "Test 7: Merge conflict detection and abort"

TASK_ID2="conflict-002"
WT_PATH2="${WORKTREE_BASE}/task-${TASK_ID2}"
BRANCH2="feature/task-${TASK_ID2}"

mkdir -p "$WORKTREE_BASE"
git worktree add "$WT_PATH2" -b "$BRANCH2" main >/dev/null 2>&1

# Change README.md in worktree
echo "worktree change" > "$WT_PATH2/README.md"
(cd "$WT_PATH2" && git add . && git commit -m "Worktree change" >/dev/null 2>&1)

# Make conflicting change on main
echo "main change" > "$PROJECT/README.md"
git add . && git commit -m "Main change" >/dev/null 2>&1

# Attempt merge (should fail)
set +e
MERGE_OUTPUT=$(git merge --no-ff -m "Merge test" "$BRANCH2" 2>&1)
MERGE_EXIT=$?
set -e

if [ $MERGE_EXIT -ne 0 ]; then
    pass "Merge correctly detected as conflict"
else
    fail "Merge should have conflicted but succeeded"
fi

if echo "$MERGE_OUTPUT" | grep -qi "conflict"; then
    pass "Conflict message present in output"
else
    fail "No conflict message in output: $MERGE_OUTPUT"
fi

# Parse conflict files
if echo "$MERGE_OUTPUT" | grep -q "README.md"; then
    pass "Conflicting file (README.md) identified"
else
    fail "Conflicting file not identified in output"
fi

# Abort merge
git merge --abort >/dev/null 2>&1
ABORT_OK=$?

if [ $ABORT_OK -eq 0 ]; then
    pass "Merge abort successful"
else
    fail "Merge abort failed"
fi

# Verify main is clean after abort
MAIN_STATUS=$(git status --porcelain)
if [ -z "$MAIN_STATUS" ]; then
    pass "Main branch clean after merge abort"
else
    fail "Main branch dirty after abort: $MAIN_STATUS"
fi

# Clean up
git worktree remove "$WT_PATH2" --force >/dev/null 2>&1
git worktree prune >/dev/null 2>&1
git branch -D "$BRANCH2" >/dev/null 2>&1

# ── Test 8: Orphaned worktree cleanup ─────────────────────────────────────
echo ""
echo "Test 8: Orphaned worktree cleanup"

TASK_ID3="orphan-003"
WT_PATH3="${WORKTREE_BASE}/task-${TASK_ID3}"
BRANCH3="feature/task-${TASK_ID3}"

mkdir -p "$WORKTREE_BASE"
git worktree add "$WT_PATH3" -b "$BRANCH3" main >/dev/null 2>&1

if [ -d "$WT_PATH3" ]; then
    pass "Orphan worktree created for test"
else
    fail "Could not create orphan test worktree"
fi

# Simulate cleanup: prune + remove directories + delete branches
git worktree prune >/dev/null 2>&1

for dir in "$WORKTREE_BASE"/task-*; do
    [ -d "$dir" ] || continue
    TNAME=$(basename "$dir")
    TID=${TNAME#task-}
    TBRANCH="feature/task-${TID}"

    # Remove node_modules symlink
    rm -f "$dir/node_modules" 2>/dev/null

    # Remove via git
    git worktree remove "$dir" --force >/dev/null 2>&1 || rm -rf "$dir"

    # Delete branch
    git branch -D "$TBRANCH" >/dev/null 2>&1 || true
done

git worktree prune >/dev/null 2>&1

if [ ! -d "$WT_PATH3" ]; then
    pass "Orphaned worktree cleaned up"
else
    fail "Orphaned worktree still exists"
fi

if ! git branch | grep -q "$BRANCH3"; then
    pass "Orphaned branch cleaned up"
else
    fail "Orphaned branch still exists"
fi

# ── Test 9: Concurrent worktrees ─────────────────────────────────────────
echo ""
echo "Test 9: Concurrent worktrees (parallel isolation)"

TASK_A="concurrent-a"
TASK_B="concurrent-b"
WT_A="${WORKTREE_BASE}/task-${TASK_A}"
WT_B="${WORKTREE_BASE}/task-${TASK_B}"
BR_A="feature/task-${TASK_A}"
BR_B="feature/task-${TASK_B}"

mkdir -p "$WORKTREE_BASE"
git worktree add "$WT_A" -b "$BR_A" main >/dev/null 2>&1
git worktree add "$WT_B" -b "$BR_B" main >/dev/null 2>&1

if [ -d "$WT_A" ] && [ -d "$WT_B" ]; then
    pass "Two concurrent worktrees created"
else
    fail "Failed to create concurrent worktrees"
fi

# Make different changes in each
echo "file from A" > "$WT_A/fileA.txt"
(cd "$WT_A" && git add . && git commit -m "Add fileA" >/dev/null 2>&1)

echo "file from B" > "$WT_B/fileB.txt"
(cd "$WT_B" && git add . && git commit -m "Add fileB" >/dev/null 2>&1)

# Verify isolation: A doesn't have B's file and vice versa
if [ ! -f "$WT_A/fileB.txt" ] && [ ! -f "$WT_B/fileA.txt" ]; then
    pass "Worktrees are isolated (no cross-contamination)"
else
    fail "Worktree isolation broken"
fi

# Merge both sequentially
git merge --no-ff -m "Merge A" "$BR_A" >/dev/null 2>&1
git merge --no-ff -m "Merge B" "$BR_B" >/dev/null 2>&1

if [ -f "$PROJECT/fileA.txt" ] && [ -f "$PROJECT/fileB.txt" ]; then
    pass "Both merges completed, files on main"
else
    fail "Merge of concurrent worktrees failed"
fi

# Cleanup
git worktree remove "$WT_A" --force >/dev/null 2>&1
git worktree remove "$WT_B" --force >/dev/null 2>&1
git worktree prune >/dev/null 2>&1
git branch -d "$BR_A" >/dev/null 2>&1
git branch -d "$BR_B" >/dev/null 2>&1

# ── Summary ───────────────────────────────────────────────────────────────
echo ""
echo "========================================="
echo " Results: $PASS passed, $FAIL failed"
echo "========================================="

if [ $FAIL -gt 0 ]; then
    echo ""
    echo "Failures:"
    echo -e "$ERRORS"
    exit 1
fi

echo ""
echo "All tests passed!"
exit 0
