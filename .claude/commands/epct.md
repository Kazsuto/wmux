---
description: Systematic implementation using Explore-Plan-Code-Test methodology
---

<objective>
Implement features systematically using the EPCT workflow: Explore, Plan, Code, Test.
</objective>

<process>
## 1. EXPLORE
Use Task tool with `subagent_type="Explore"` (thoroughness: "very thorough") to:
- Find relevant files for implementation
- Identify patterns and conventions
- Gather context from codebase

Use `explore-docs` agent if library documentation needed.

## 2. PLAN

- Write comprehensive implementation plan
- Include core changes, tests, and documentation updates
- **STOP and ASK** user if anything unclear

## 3. CODE

- Follow existing codebase patterns
- Stay strictly in scope
- No unnecessary comments
- Run `pnpm lint` and `pnpm format` when done

## 4. TEST

- Run `pnpm lint` and `pnpm ts`
- Run only tests related to changes: `pnpm test:ci <pattern>`
- If tests fail: return to PLAN phase
  </process>

<success_criteria>

- All relevant files discovered
- Implementation follows existing patterns
- Tests pass for changed code
- Linting and type checking pass
  </success_criteria>

---

User: #$ARGUMENTS
