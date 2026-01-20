# Vibe-Kanban Integration for Agent Skills

## Overview

This document describes how to update the agent skills in `.claude/skills` to integrate with vibe-kanban task management. Vibe-kanban provides project and task tracking tools that complement the existing workflow skills.

## Available Vibe-Kanban Tools

| Tool | Purpose |
|------|---------|
| `list_projects` | List all available projects |
| `list_tasks` | List tasks in a project with optional status filter |
| `get_task` | Get detailed task information including description |
| `create_task` | Create a new task in a project |
| `update_task` | Update task title, description, or status |
| `delete_task` | Delete a task |
| `list_repos` | List repositories for a project |
| `get_repo` | Get repository details and scripts |
| `start_workspace_session` | Start working on a task with a new workspace |

### Task Statuses
- `todo` - Not started
- `inprogress` - Currently being worked on
- `inreview` - Awaiting review
- `done` - Completed
- `cancelled` - No longer needed

---

## Skills Requiring Integration

### 1. brainstorming (HIGH PRIORITY)

**Current behavior:** Creates design documents, then hands off to worktree/plan skills.

**Integration points:**
- After design approval, **create a vibe-kanban task** for the feature
- Link the task to the created design document

**Proposed changes to `brainstorming/SKILL.md`:**

```markdown
## After the Design

**Task Creation (if using vibe-kanban):**
- Check if project has an associated vibe-kanban project (list_projects)
- Create task: `create_task(project_id, title="<feature-name>", description="See: docs/plans/YYYY-MM-DD-<topic>-design.md")`
- Include task ID in subsequent handoffs

**Documentation:**
- Write the validated design to `docs/plans/YYYY-MM-DD-<topic>-design.md`
...
```

---

### 2. writing-plans (HIGH PRIORITY)

**Current behavior:** Creates detailed implementation plans saved to `docs/plans/`.

**Integration points:**
- Plans can be linked to existing vibe-kanban tasks
- Each plan task can be created as a vibe-kanban subtask
- Task status updated when plan is complete

**Proposed changes to `writing-plans/SKILL.md`:**

Add after "Plan Document Header" section:

```markdown
## Vibe-Kanban Integration

**If working on a vibe-kanban task:**

1. **Link plan to task:**
   - Get task details: `get_task(task_id)`
   - Update task with plan reference: `update_task(task_id, description="Plan: docs/plans/<filename>.md\n\n<original description>")`

2. **Create subtasks for each plan task (optional):**
   - For larger plans, create vibe-kanban tasks for each major component
   - Use: `create_task(project_id, title="Task N: <component>", description="Part of <parent-task>")`

3. **Update task status:**
   - When plan is written: `update_task(task_id, status="inprogress")`
```

Update "Execution Handoff" section:

```markdown
## Execution Handoff

After saving the plan:

**If vibe-kanban task exists:**
- Update task status to `inprogress`
- Note task_id for execution tracking

**Then offer execution choice:**
...
```

---

### 3. executing-plans (HIGH PRIORITY)

**Current behavior:** Executes plans in batches with review checkpoints.

**Integration points:**
- Track progress against vibe-kanban task
- Update task status as batches complete
- Mark task done when execution completes

**Proposed changes to `executing-plans/SKILL.md`:**

Add new section after "Overview":

```markdown
## Vibe-Kanban Task Tracking

**If executing against a vibe-kanban task:**

1. **At start:**
   - Verify task exists: `get_task(task_id)`
   - Confirm task status is `todo` or `inprogress`
   - Update to `inprogress` if not already: `update_task(task_id, status="inprogress")`

2. **During execution:**
   - Progress tracked via TodoWrite (local)
   - No per-step vibe-kanban updates (too noisy)

3. **At completion:**
   - Update task to `inreview`: `update_task(task_id, status="inreview")`
   - Include summary in task description if desired

4. **After final review approved:**
   - Update task to `done`: `update_task(task_id, status="done")`
```

Modify "Step 5: Complete Development":

```markdown
### Step 5: Complete Development

After all tasks complete and verified:
- **If vibe-kanban task:** Update status to `inreview`: `update_task(task_id, status="inreview")`
- Announce: "I'm using the finishing-a-development-branch skill to complete this work."
- **REQUIRED SUB-SKILL:** Use superpowers:finishing-a-development-branch
- Follow that skill to verify tests, present options, execute choice
- **After merge/PR:** Update task to `done`: `update_task(task_id, status="done")`
```

---

### 4. subagent-driven-development (HIGH PRIORITY)

**Current behavior:** Executes plans via subagents with two-stage review.

**Integration points:**
- Same as executing-plans: track task status
- Update to `inreview` after all tasks + final code review
- Update to `done` after finishing-a-development-branch completes

**Proposed changes to `subagent-driven-development/SKILL.md`:**

Add after "When to Use":

```markdown
## Vibe-Kanban Task Tracking

**If working on a vibe-kanban task:**

1. **At start:**
   - Verify task: `get_task(task_id)`
   - Update to `inprogress`: `update_task(task_id, status="inprogress")`

2. **During execution:**
   - Per-task progress tracked via TodoWrite
   - Vibe-kanban task status remains `inprogress`

3. **After final code review:**
   - Update to `inreview`: `update_task(task_id, status="inreview")`

4. **After finishing-a-development-branch (merge/PR created):**
   - Update to `done`: `update_task(task_id, status="done")`
```

Update process diagram to include vibe-kanban status updates at key transitions.

---

### 5. finishing-a-development-branch (HIGH PRIORITY)

**Current behavior:** Presents options (merge, PR, keep, discard) and executes choice.

**Integration points:**
- Update vibe-kanban task status based on choice
- Link PR URL to task when creating PR

**Proposed changes to `finishing-a-development-branch/SKILL.md`:**

Add after "Step 1: Verify Tests":

```markdown
### Step 1.5: Check Vibe-Kanban Task (if applicable)

If working on a vibe-kanban task:
- Verify task exists and is `inprogress` or `inreview`
- Task ID should have been passed from previous skill
```

Modify "Step 4: Execute Choice":

```markdown
#### Option 1: Merge Locally

...existing steps...

**Vibe-kanban (if applicable):**
- Update task: `update_task(task_id, status="done")`

#### Option 2: Push and Create PR

...existing steps...

**Vibe-kanban (if applicable):**
- Update task description with PR URL
- Update task: `update_task(task_id, status="inreview")`
- Note: Task moves to `done` when PR is merged (manual or via CI)

#### Option 4: Discard

...existing steps...

**Vibe-kanban (if applicable):**
- Update task: `update_task(task_id, status="cancelled")`
```

---

### 6. using-git-worktrees (MEDIUM PRIORITY)

**Current behavior:** Creates isolated worktrees for feature work.

**Integration points:**
- Can use `start_workspace_session` as alternative to manual worktree creation
- Links worktree to vibe-kanban task for tracking

**Proposed changes to `using-git-worktrees/SKILL.md`:**

Add new section:

```markdown
## Vibe-Kanban Workspace Sessions

**Alternative to manual worktree creation:**

If working on a vibe-kanban task, you can use `start_workspace_session` to:
- Automatically create workspace linked to task
- Run repository setup scripts
- Track workspace in vibe-kanban

```bash
# Instead of manual worktree:
start_workspace_session(
  task_id="<task-uuid>",
  executor="CLAUDE_CODE",
  repos=[{repo_id: "<repo-uuid>", base_branch: "main"}]
)
```

**When to use manual worktrees vs vibe-kanban sessions:**
- **Manual worktrees:** Quick experiments, no task tracking needed
- **Vibe-kanban sessions:** Formal task execution with tracking
```

---

### 7. requesting-code-review (MEDIUM PRIORITY)

**Current behavior:** Dispatches code reviewer subagent.

**Integration points:**
- When review is requested, can update task to `inreview`
- Review feedback can be noted in task description

**Proposed changes to `requesting-code-review/SKILL.md`:**

Add to "When to Request Review":

```markdown
**Vibe-kanban integration:**
- When requesting review, optionally update task status:
  `update_task(task_id, status="inreview")`
- After review approved, status flows through finishing-a-development-branch
```

---

### 8. dispatching-parallel-agents (LOW PRIORITY)

**Current behavior:** Dispatches multiple agents for independent problems.

**Integration points:**
- Could create vibe-kanban subtasks for each parallel investigation
- Generally too granular for vibe-kanban tracking

**Proposed changes:** Minimal - add note about optional task creation:

```markdown
## Vibe-Kanban Integration (Optional)

For long-running parallel investigations, consider creating vibe-kanban subtasks:
- `create_task(project_id, title="Investigate: <problem-domain>")`
- Update to `done` when agent returns successful fix

Generally, parallel agent work is too granular for vibe-kanban tracking.
```

---

## Skills NOT Requiring Integration

The following skills don't need vibe-kanban integration:

| Skill | Reason |
|-------|--------|
| `systematic-debugging` | Technical process, not task management |
| `test-driven-development` | Code practice, not workflow |
| `verification-before-completion` | Quality gate, operates below task level |
| `receiving-code-review` | Response process, not initiation |
| `writing-skills` | Meta-skill for skill authoring |
| `using-superpowers` | Meta-skill for skill discovery |

---

## Implementation Summary

### High Priority (Update First)
1. **finishing-a-development-branch** - Final status updates
2. **executing-plans** - Task status throughout execution
3. **subagent-driven-development** - Task status throughout execution
4. **writing-plans** - Link plans to tasks
5. **brainstorming** - Create tasks from designs

### Medium Priority
6. **using-git-worktrees** - Document `start_workspace_session` alternative
7. **requesting-code-review** - Status update on review request

### Low Priority
8. **dispatching-parallel-agents** - Optional subtask creation

---

## Cross-Cutting Concerns

### Task ID Propagation

Skills that hand off to other skills must pass `task_id`:

```
brainstorming → writing-plans → executing-plans → finishing-a-development-branch
                              → subagent-driven-development → finishing-a-development-branch
```

Each skill should:
1. Accept optional `task_id` parameter/context
2. Pass `task_id` to downstream skills
3. Use `task_id` for vibe-kanban updates

### Error Handling

If vibe-kanban tools fail:
- Log warning but don't block workflow
- Vibe-kanban is enhancement, not requirement
- Core skill functionality must work without vibe-kanban

### Status Flow

```
todo → inprogress → inreview → done
                  ↘ cancelled
```

| Transition | Triggered By |
|------------|--------------|
| `todo` → `inprogress` | Plan execution starts |
| `inprogress` → `inreview` | Code review requested / PR created |
| `inreview` → `done` | Merge completed |
| Any → `cancelled` | Work discarded |

---

## Example Workflow

```
1. User: "Add user authentication feature"

2. brainstorming skill:
   - Refines requirements
   - Creates design doc: docs/plans/2026-01-20-auth-design.md
   - Creates vibe-kanban task: "Add user authentication"
   - Hands off to writing-plans with task_id

3. writing-plans skill:
   - Creates implementation plan
   - Links plan to vibe-kanban task
   - Updates task status: inprogress
   - Hands off to subagent-driven-development with task_id

4. subagent-driven-development skill:
   - Executes plan tasks via subagents
   - Tracks progress via TodoWrite
   - After final review: updates task status: inreview
   - Hands off to finishing-a-development-branch with task_id

5. finishing-a-development-branch skill:
   - Verifies tests pass
   - User chooses: "Create PR"
   - Creates PR, links to task
   - Task status remains: inreview

6. [Later, when PR merged]
   - Task status: done (manual or via CI hook)
```

---

## Next Steps

1. Review this document with team
2. Prioritize which skills to update first
3. Update skills incrementally, test each
4. Consider adding vibe-kanban context to skill prompts
