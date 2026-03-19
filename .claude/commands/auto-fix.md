---
description: Automated workflow to fix all ESLint and TypeScript errors with parallel processing
allowed-tools: Bash(pnpm :*), Read, Task, Grep
---

<objective>
Automatically fix all ESLint and TypeScript errors by discovering issues and processing them in parallel using Snipper agents.
</objective>

<process>
1. **Discover commands**: Check `package.json` for lint/typecheck scripts
2. **Run diagnostics**: Execute `pnpm lint` and `pnpm ts`
3. **Analyze errors**: Group errors by file (max 5 files per group)
4. **Parallel processing**: Launch Snipper agents for each group via Task tool
5. **Verify**: Re-run diagnostics after fixes
6. **Format**: Run `pnpm format` if available
</process>

<agent_instructions>
For each file group, launch a Snipper agent with:

```
Fix all ESLint and TypeScript errors in these files:
[list of files with their specific errors]

Make minimal changes to fix errors while preserving functionality.
```

</agent_instructions>

<success_criteria>

- All lint errors fixed
- All TypeScript errors resolved
- Code formatted consistently
  </success_criteria>
