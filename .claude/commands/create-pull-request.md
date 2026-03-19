---
description: Create and push PR with auto-generated title and description
model: haiku
allowed-tools: Bash(git :*), Bash(gh :*)
---

<objective>
Create a pull request with a concise, meaningful description from current branch changes.
</objective>

<context>
- Current branch: !`git branch --show-current`
- Working tree status: !`git status --short`
- Recent commits: !`git log --oneline -5`
</context>

<process>
1. **Verify state**: Check not on main/master branch
2. **Push branch**: `git push -u origin HEAD`
3. **Analyze changes**: `git diff origin/main...HEAD --stat`
4. **Create PR**: `gh pr create --title "..." --body "..."`
5. **Return PR URL**
</process>

<pr_format>
Title: One-line summary (max 72 chars)
Body:

```markdown
## Summary

- [Main change or feature]
- [Secondary changes]

## Type

[feat/fix/refactor/docs/chore]
```

</pr_format>

<success_criteria>

- Branch pushed to remote
- PR created with clear title and bullet-point summary
- PR URL returned to user
  </success_criteria>
