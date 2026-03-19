---
description: Fetch PR review comments and implement all requested changes
allowed-tools: Bash(gh :*), Bash(git :*), Read, Edit
---

<objective>
Systematically address ALL unresolved PR review comments.
</objective>

<context>
- Current branch: !`git branch --show-current`
- Working tree status: !`git status --short`
</context>

<process>
1. **Fetch comments**:
   - `gh pr status --json number,headRefName`
   - `gh api repos/{owner}/{repo}/pulls/{number}/comments`
   - Capture both review comments AND inline code comments

2. **Analyze & plan**:
   - Extract file:line references
   - Group changes by file
   - Create checklist of comments to address

3. **Implement fixes**:
   - **ALWAYS** Read target file before editing
   - Make exactly what reviewer requested
   - Track progress through checklist

4. **Commit & push**:
   - `git add -A`
   - `git commit -m "fix: address PR review comments"`
   - `git push`
     </process>

<rules>
- Read files before editing - no exceptions
- Stay in scope - only fix what reviewers requested
- No "Generated with Claude Code" or co-author tags
</rules>

<success_criteria>

- Every unresolved comment addressed
- Changes committed and pushed
- No scope creep beyond review requests
  </success_criteria>
