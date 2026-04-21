# Feature Specification: SF CLI Task Templates

**Feature Branch**: `001-sf-cli-task-templates`
**Created**: 2026-04-17
**Status**: Draft
**Input**: User description: "Add sf CLI commands as Zed task templates so developers can run deploy, retrieve, test, org management, and data operations from within Zed via task:spawn command palette"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Deploy Source to Org (Priority: P1)

A Salesforce developer has made changes to Apex classes and wants to deploy them to their default org without leaving Zed.

**Why this priority**: Deploying source is the most frequent operation in the Salesforce development loop. Every developer needs this multiple times per day.

**Independent Test**: Open a Salesforce DX project in Zed, open any `.cls` file, invoke task:spawn, select "sf: project deploy start", and verify the deploy runs in the integrated terminal.

**Acceptance Scenarios**:

1. **Given** a Salesforce DX project is open in Zed, **When** the developer invokes task:spawn and selects "sf: project deploy start", **Then** the sf CLI deploy command runs in the integrated terminal from the project root.
2. **Given** a deploy is in progress, **When** the developer attempts to start another deploy, **Then** the system prevents concurrent deploy runs.

---

### User Story 2 - Run Apex Tests (Priority: P1)

A developer wants to run Apex tests — either all local tests, a specific test class, or a specific test method — from within Zed.

**Why this priority**: Test execution is tightly coupled with the deploy cycle and is essential for validating changes before committing.

**Independent Test**: Open a test class file, invoke task:spawn, and verify each test scope (all local, current file, specific method via inline run button) executes correctly.

**Acceptance Scenarios**:

1. **Given** a Salesforce DX project is open, **When** the developer selects "sf: apex run test (all local)", **Then** all local tests run with code coverage output in the terminal.
2. **Given** a test class file is open, **When** the developer selects "sf: apex run test (current file)", **Then** tests for that specific class run using the file name as the class identifier.
3. **Given** a test method is annotated with `@isTest` or `testMethod`, **When** the developer clicks the inline run button on the method, **Then** that specific test method runs via `sf apex run test --tests ClassName.MethodName`.

---

### User Story 3 - Retrieve Source from Org (Priority: P2)

A developer wants to pull metadata changes from their org back to the local project.

**Why this priority**: Retrieve complements deploy as the second half of the source sync cycle.

**Independent Test**: Invoke task:spawn, select "sf: project retrieve start", and verify metadata is pulled to the local project.

**Acceptance Scenarios**:

1. **Given** a Salesforce DX project is open, **When** the developer selects "sf: project retrieve start", **Then** the retrieve command runs from the project root and outputs results in the terminal.

---

### User Story 4 - Org Management (Priority: P2)

A developer wants to open their org in a browser, list authenticated orgs, create scratch orgs, or authenticate to new orgs — all from within Zed.

**Why this priority**: Org management tasks are frequent context switches that benefit from editor integration.

**Independent Test**: Invoke task:spawn, select an org management task (e.g., "sf: org open"), and verify the expected action occurs.

**Acceptance Scenarios**:

1. **Given** a default org is configured, **When** the developer selects "sf: org open", **Then** the default org opens in the system browser and the terminal does not steal focus.
2. **Given** the developer needs to see authenticated orgs, **When** they select "sf: org list", **Then** the org list displays in the terminal.

---

### User Story 5 - Execute Anonymous Apex and Data Queries (Priority: P3)

A developer wants to run anonymous Apex from the current file or execute SOQL/SOSL queries using selected text.

**Why this priority**: These are diagnostic/exploratory tasks that enhance developer productivity but are not part of the core deploy-test loop.

**Independent Test**: Open an `.apex` file and run it via task:spawn; select a SOQL query string in an Apex file and run it via the data query task.

**Acceptance Scenarios**:

1. **Given** an anonymous Apex file is open, **When** the developer selects "sf: apex run (current file)", **Then** the file content executes against the default org.
2. **Given** a SOQL query string is selected in the editor, **When** the developer selects "sf: data query (selection)", **Then** the query runs and results display in the terminal.

---

### Edge Cases

- What happens when no default org is configured? The sf CLI surfaces its own error message in the terminal; the extension does not need to pre-validate.
- What happens when the sf CLI is not installed? The task fails with a "command not found" error in the terminal.
- What happens when `$ZED_SELECTED_TEXT` is empty for data query tasks? The sf CLI receives an empty query argument and returns its own error.
- What happens when the file stem does not match the Apex class name? The test-by-class task may target the wrong class. This follows Salesforce convention where file name MUST match class name.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Extension MUST provide task templates for sf CLI commands accessible via Zed's task:spawn mechanism.
- **FR-002**: Tasks MUST run from the project root (`$ZED_WORKTREE_ROOT`) to match sf CLI expectations.
- **FR-003**: Deploy and retrieve tasks MUST prevent concurrent runs to avoid org conflicts.
- **FR-004**: Test execution tasks MUST support three scopes: all local tests, current file class, and specific method.
- **FR-005**: The inline run button (from `runnables.scm`) MUST trigger method-level test execution for `@isTest` and `testMethod` annotations.
- **FR-006**: Read-only tasks (list, display, query) MUST allow concurrent runs.
- **FR-007**: Org-opening tasks MUST not steal terminal focus from the editor.
- **FR-008**: Data query and search tasks MUST use the current editor selection as the query argument.
- **FR-009**: Log tailing MUST open in a new terminal tab to support long-running observation.
- **FR-010**: All tasks MUST be prefixed with "sf: " for discoverability and namespace clarity.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: All defined sf CLI tasks appear in the task:spawn modal when an Apex file is open.
- **SC-002**: Deploy, retrieve, and test tasks complete successfully against a configured Salesforce org.
- **SC-003**: Inline run buttons appear on `@isTest` and `testMethod` annotated methods and trigger the correct test execution.
- **SC-004**: No task introduces a regression in existing extension functionality (LSP startup, syntax highlighting).
- **SC-005**: Developer can complete a full deploy-test-retrieve cycle without leaving Zed.

## Assumptions

- The `sf` CLI (v2+) is installed and available on the developer's system PATH.
- A default target org is configured via `sf config set target-org` or equivalent.
- The project follows Salesforce DX source-format conventions with `sfdx-project.json` at the worktree root.
- Apex class file names match their class names (standard Salesforce convention).
- Zed's task template system loads tasks from `languages/<lang>/tasks.json` when a file of that language is open.
