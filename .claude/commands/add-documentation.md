---
description: Add or update documentation in content/docs/ for features, APIs, or tools
allowed-tools: Read, Write, Edit, Glob, Grep, WebFetch, AskUserQuestion
---

<objective>
Create or update documentation files in `content/docs/` for any feature, API, tool, or guide. This command helps you gather all necessary context (source files, videos, resources) before writing comprehensive documentation.
</objective>

<overview>
Documentation in NOW.TS is stored in `content/docs/` as `.mdx` files with YAML front-matter. The docs system supports:
- **Guides**: Step-by-step tutorials (tags: ["guide"])
- **API Documentation**: Endpoint documentation with code examples (tags: ["api"])
- **Component Documentation**: UI component usage guides (tags: ["components"])
- **Reference**: Technical reference documentation (tags: ["reference"])
</overview>

<documentation_structure>

## Front-matter Schema

```yaml
---
title: "Page Title"
description: "Brief description of what this doc covers"
keywords: ["keyword1", "keyword2", "keyword3"]
tags: ["guide" | "api" | "components" | "reference"]
order: 1 # For ordering in sidebar
subcategory: "Category Name" # For grouping in sidebar

# API docs only:
method: "GET" | "POST" | "PATCH" | "DELETE" | "PUT"
endpoint: "/v1/resource"
examples:
  bash: |
    curl command here...
  javascript: |
    fetch example here...
  python: |
    requests example here...
results:
  success: |
    { "response": "example" }
  error: |
    { "error": "example" }
---
```

## Content Structure

For guides:

1. Introduction paragraph
2. Quick Setup / Prerequisites (if applicable)
3. Step-by-step sections with ## headings
4. Code examples in fenced blocks
5. Next Steps / Related docs

For API docs:

1. Brief description
2. Endpoint section
3. Authentication section
4. Request Body table
5. Response table
6. Error Codes table

For component docs:

1. Installation (if needed)
2. Import statement
3. Basic usage example
4. Props/Options table
5. Advanced patterns
6. Best practices

</documentation_structure>

<process>

## Step 1: Gather Context

Ask the user for ALL necessary information:

1. **What to document?**
   - Feature name / API endpoint / Component name
   - Type: Guide, API, Component, or Reference

2. **Source files** (CRITICAL - read these first!):
   - For components: Component source file(s)
   - For APIs: Route handler file, types, validation schemas
   - For features: All related source files

3. **Additional resources**:
   - Video tutorials (YouTube URL for WebFetch)
   - External documentation links
   - Design docs or specifications

## Step 2: Analyze Sources

1. Read ALL provided source files
2. Extract:
   - Function signatures and types
   - Props/Options with their types
   - Usage patterns from existing code
   - Error handling
   - Edge cases

## Step 3: Check Existing Documentation

1. Glob `content/docs/*.mdx` to see existing docs
2. Check if doc already exists (update vs create)
3. Look at similar docs for style consistency

## Step 4: Write Documentation

1. Create/update the `.mdx` file in `content/docs/`
2. Follow the front-matter schema
3. Match the tone/style of existing docs
4. Include practical code examples
5. Add cross-references to related docs

## Step 5: Validate

1. Verify front-matter is valid YAML
2. Check all code examples are syntactically correct
3. Ensure all referenced docs/links exist

</process>

<questions_to_ask>

Ask these questions using AskUserQuestion:

1. **Doc Type**
   - Question: "What type of documentation do you want to create?"
   - Options:
     - Guide (step-by-step tutorial)
     - API (endpoint documentation)
     - Component (UI component usage)
     - Reference (technical reference)

2. **Source Files**
   - Question: "Which source files should I read to understand this feature?"
   - Ask for specific file paths or patterns

3. **Target Audience**
   - Question: "Who is the target audience for this documentation?"
   - Options:
     - Developers (technical)
     - End users (non-technical)
     - Both

4. **External Resources** (optional)
   - Question: "Do you have any external resources (videos, docs, links) I should reference?"
   - Accept URLs for WebFetch

</questions_to_ask>

<file_naming>

Use kebab-case for file names:

- `getting-started.mdx`
- `api-testimonials-create.mdx`
- `dialog-manager.mdx`
- `layout-components.mdx`

Convention: `{type}-{subject}.mdx` for API docs
Convention: `{subject}.mdx` for guides/components

</file_naming>

<rules>
- ALWAYS read source files before writing documentation
- ALWAYS check existing docs in `content/docs/` for style consistency
- Use French for content if existing docs are in French
- Include practical, copy-paste-ready code examples
- Link to related documentation when relevant
- Keep examples minimal but complete
- Use tables for props/options/parameters
- Include error handling and edge cases
- Test all code examples mentally for correctness
</rules>

<success_criteria>

- Documentation file created/updated in `content/docs/`
- Valid YAML front-matter
- Consistent style with existing docs
- Practical code examples included
- All referenced source files were read
- Cross-references to related docs added
  </success_criteria>
