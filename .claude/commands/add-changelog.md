---
description: Create a new public changelog entry with proper frontmatter and structure
allowed-tools: Read, Write, Bash
---

<objective>
Create a new changelog entry MDX file in `content/changelog/` with today's date.
</objective>

<process>
1. **Gather Info**: Ask for version number (optional) and release title (optional)
2. **Generate File**: Create MDX file with today's date as filename
3. **Add Content**: Include frontmatter and template sections
4. **Confirm**: Show the created file path
</process>

<file_template>
Location: `content/changelog/YYYY-MM-DD-vXYZ.mdx` (e.g., `2025-12-15-v200.mdx` for version 2.0.0)

```mdx
---
date: YYYY-MM-DD
version: "[VERSION]" # e.g., "2.0.0" (required for filename)
title: "[TITLE]" # Optional, e.g., "Major Performance Release"
image: null # Optional, path to hero image
status: published # or "draft"
---

## What's New

[Brief summary of this release]

### Features

- **Feature name**: Description of the feature

### Bug Fixes

- Description of the fix

### Improvements

- Description of the improvement

### Breaking Changes

- Description of breaking change (if any)
```

</file_template>

<rules>
- Filename format: `YYYY-MM-DD-vXYZ.mdx` where XYZ is version without dots (e.g., v210 for 2.1.0)
- Version follows semantic versioning (e.g., 1.2.0)
- Title should be descriptive but concise
- Status defaults to "published"
- Keep sections that have content, remove empty ones
</rules>

<example>
User: "Create a changelog for version 2.1.0 with title 'Dark Mode Support'"

Creates: `content/changelog/2025-12-26-v210.mdx`

```mdx
---
date: 2025-12-26
version: "2.1.0"
title: "Dark Mode Support"
image: null
status: published
---

## What's New

This release introduces dark mode support.

### Features

- **Dark mode**: Full dark mode support across all pages

### Improvements

- Better contrast ratios for accessibility
```

</example>

<success_criteria>

- File created at correct path
- Valid YAML frontmatter
- Date matches today
- Content sections are ready for editing
  </success_criteria>
