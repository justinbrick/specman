# Migration Prompt - Specification Anchoring

You are migrating non-SpecMan code and resources into SpecMan artifacts for the target specification {{target_path}}.

Steps (follow in order):

1. Call the MCP tool `create_artifact` to create the specification artifact for this migration (target: {{target_path}}). Preserve every HTML comment directive in the generated template until the instruction is satisfied.
2. Immediately call `create_artifact` again to create a revision scratch pad targeting that specification (work type: `revision`; target: the specification handle or path). Use the canonical scratch pad location and do not begin migration work until the scratch pad exists.
3. Call `create_artifact` to create an implementation targeting that specification. Keep HTML directives intact and align the implementation scope with the specification you just created.
4. Call `create_artifact` to create a feature scratch pad targeting the implementation. Fill that scratch pad with a plan for reading the source code and documenting the implementation in accordance with the specification; do not edit any code while doing this scratch work.
5. Execute the migration phases in sequence, recording progress in the scratch pad, specification, and implementation materials:
   - Phase 1 - Enumerate sources: list all source files, modules, and assets that must be scanned.
   - Phase 2 - Extract findings: read the codebase and capture candidate concepts, entities, and constraints; note uncertainties as questions instead of guessing.
   - Phase 3 - Draft/update specification: produce or revise the specification using the findings, keeping HTML directives intact and mapping back to dependencies.
   - Phase 4 - Generate implementation documentation: after the specification draft is produced, outline implementation notes and traceability for downstream work.
6. Do not reference or link to other scratch pads; duplicate any necessary instructions inline. Keep the output textual-only.

## User Input

- Provide migration context and goals here. Keep this section at the bottom so user input stays isolated from the prompt structure.
