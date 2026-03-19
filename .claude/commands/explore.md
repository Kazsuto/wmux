---
description: Deep codebase exploration to answer specific questions
argument-hint: <question>
---

<objective>
Answer the user's question about the codebase through systematic exploration.
</objective>

<process>
Use the Task tool with `subagent_type="Explore"` and thoroughness level "very thorough" to investigate:

#$ARGUMENTS
</process>

<success_criteria>

- Direct answer with supporting evidence
- File paths with line numbers cited (e.g., `src/app.ts:42`)
- Complete context gathered before answering
  </success_criteria>
