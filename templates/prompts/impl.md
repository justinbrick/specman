# Implementation Creation

You are documenting an implementation and must use the MCP tool `create_artifact` to instantiate the canonical implementation template, ensuring it complies with the SpecMan Data Model and realizes the specification at {{target_path}} while keeping every HTML comment directive until fulfilled.

Before doing anything else, complete these prerequisites:

- Study the SpecMan Data Model specification plus its declared dependencies (for example the Founding Spec) so you apply the latest implementation rules.
- Open the governing specification at {{target_spec_path}} and read every dependency listed inside it to understand upstream constraints.
- Review the existing implementation materials referenced in the provided context, along with every item in the implementation's `references` list, so you know all downstream contracts.

Read the following dependencies before continuing:
{{context}}

Steps:

1. Decide the implementation name and ensure it matches the governing naming rules.
2. Call the MCP tool `create_artifact` to create the Implementation, then set `spec` to {{target_spec_path}}, fill `location`, `library` (if applicable), and describe `primary_language` plus optional `secondary_languages`, replacing the references list with {{reference_items}}.
3. Summarize architecture and intent in Overview, explain language details, References, Implementation Details, and Operational Notes with concise prose that cites specification headings via inline links, grounding content in the User Input section.
4. In `## Concept & Entity Breakdown`, enumerate every concept and entity from the governing specification that this implementation covers. Each heading MUST include an inline link to the originating spec fragment (for example `[Concept: Lifecycle](../spec/spec.md#concept-lifecycle)`) and contain:

   - A narrative describing how the concept/entity is realized, with additional inline links to related headings.
   - API signatures scoped to that concept/entity (fenced code blocks with notes on inputs, outputs, invariants, and dependencies).
   - For entities, an embedded Data Model snippet plus any constraints.

5. Remove HTML comment directives only after their guidance is satisfied and validate against SpecMan Data Model sections for Implementations, Implementing Language, References, APIs, and Implementation Metadata before returning the result.

## User Input

- Provide the implementation requirements, scope, and constraints here. Keep this section at the bottom so user input stays isolated from the prompt structure.
