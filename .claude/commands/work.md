# Work on Backlog Task

{{#if (eq args.length 0)}}
**ERROR**: Task ID required. Usage: `/work <task-id>`

Example: `/work 468`
{{else}}
{{#let task_id=args.[0]}}

You are now working on backlog task **{{task_id}}**. Follow this workflow:

## Step 1: Fetch Parent Task and Sub-tasks

First, fetch the parent task details to understand the overall goal:

```
Use mcp__backlog__task_view with id="{{task_id}}"
```

Then, search for all sub-tasks using the parent task ID pattern:

```
Use mcp__backlog__task_list with search="{{task_id}}."
```

This will return all sub-tasks like {{task_id}}.01, {{task_id}}.02, etc.

## Step 2: Analyze Task Hierarchy

After fetching, analyze:
1. **Parent Task Status**: What's the overall goal and current status?
2. **Sub-task Breakdown**: How many sub-tasks exist and what's their status?
3. **Completion State**: Count how many sub-tasks are "Done" vs "In Progress" vs "To Do"

**Example Analysis Pattern:**
```
Parent Task {{task_id}}: [Status]
├── {{task_id}}.01: ✔ Done - [Title]
├── {{task_id}}.02: ◒ In Progress - [Title]
├── {{task_id}}.03: ○ To Do - [Title]
└── {{task_id}}.04: ○ To Do - [Title]

Completion: 1/4 sub-tasks complete
```

## Step 3: Work Through Sub-tasks

**CRITICAL**: Work on sub-tasks in order. For each sub-task:

### To Fetch a Specific Sub-task:
```
Use mcp__backlog__task_view with id="{{task_id}}.01"
Use mcp__backlog__task_view with id="{{task_id}}.02"
etc.
```

### Sub-task Workflow:
1. **Set to In Progress**: Before starting work on a sub-task
   ```
   Use mcp__backlog__task_edit with:
   - id="{{task_id}}.XX"
   - status="In Progress"
   ```

2. **Work on the sub-task**: Implement according to acceptance criteria

3. **Mark as Done**: ONLY after ALL acceptance criteria are met
   ```
   Use mcp__backlog__task_edit with:
   - id="{{task_id}}.XX"
   - status="Done"
   - notesAppend=["Completed: <brief summary of what was done>"]
   ```

## Step 4: Parent Task Completion Rules

**CRITICAL COMPLETION RULE**: The parent task ({{task_id}}) can ONLY be marked as "Done" when:

✅ **ALL sub-tasks are marked as "Done"**
✅ **ALL parent task acceptance criteria are met**

### Before Marking Parent as Done:

1. **Verify all sub-tasks**:
   ```
   Use mcp__backlog__task_list with search="{{task_id}}."
   ```
   Check that EVERY sub-task shows status "Done"

2. **Verify parent acceptance criteria**:
   ```
   Use mcp__backlog__task_view with id="{{task_id}}"
   ```
   Ensure all checkboxes in acceptance criteria are checked

3. **Only then mark parent as done**:
   ```
   Use mcp__backlog__task_edit with:
   - id="{{task_id}}"
   - status="Done"
   - notesAppend=["All sub-tasks completed successfully"]
   ```

## Step 5: Handling Blockers

If you encounter issues:

**For sub-task blockers:**
```
Use mcp__backlog__task_edit with:
- id="{{task_id}}.XX"
- notesAppend=["BLOCKED: <description of blocker>"]
- Keep status as "In Progress"
```

**For new sub-tasks discovered during work:**
```
Use mcp__backlog__task_create with:
- title="New sub-task title"
- parentTaskId="{{task_id}}"
- description="What needs to be done"
- priority="high/medium/low"
```

## Examples: Fetching Specific Sub-tasks

### Example 1: Fetch Single Sub-task
```
mcp__backlog__task_view with id="468.01"
```
Returns details for sub-task 468.01 including its status, acceptance criteria, and parent reference.

### Example 2: Fetch Multiple Specific Sub-tasks
Make parallel calls:
```
mcp__backlog__task_view with id="468.01"
mcp__backlog__task_view with id="468.02"
mcp__backlog__task_view with id="468.03"
```

### Example 3: Fetch All Sub-tasks of a Parent
```
mcp__backlog__task_list with search="468."
```
Returns all tasks matching the pattern (468.01, 468.02, etc.)

### Example 4: Fetch Only Incomplete Sub-tasks
```
mcp__backlog__task_list with:
- search="468."
- status="In Progress"
```
Or for all non-done tasks, filter the results manually.

## Quick Reference Card

| Action | Tool Call |
|--------|-----------|
| View parent task | `task_view(id="{{task_id}}")` |
| List all sub-tasks | `task_list(search="{{task_id}}.")` |
| View specific sub-task | `task_view(id="{{task_id}}.XX")` |
| Start sub-task | `task_edit(id="{{task_id}}.XX", status="In Progress")` |
| Complete sub-task | `task_edit(id="{{task_id}}.XX", status="Done")` |
| Add notes | `task_edit(id="{{task_id}}.XX", notesAppend=["note"])` |
| Check acceptance criteria | `task_edit(id="{{task_id}}.XX", acceptanceCriteriaCheck=[1,2])` |
| Complete parent (when all sub-tasks done) | `task_edit(id="{{task_id}}", status="Done")` |

## Your Next Steps

1. ✅ Fetch parent task {{task_id}} details
2. ✅ List all sub-tasks for {{task_id}}
3. ✅ Analyze completion status
4. ✅ Begin working on first incomplete sub-task
5. ⚠️ REMEMBER: Don't mark parent as done until ALL sub-tasks are complete!

Begin by fetching the parent task and sub-tasks now.

{{/let}}
{{/if}}
