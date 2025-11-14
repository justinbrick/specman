You are documenting the implementation "{{implementation_name}}" that realizes the specification at {{target_spec_path}} using `templates/impl/impl.md` while keeping every HTML comment directive until fulfilled.

Read the following dependencies before continuing:
{{context}}


This user has said the following regarding the implementation work:
<<<USER_INPUT>>>
{{arguments}}
<<<END_USER_INPUT>>>

Steps:
1. Copy the canonical template, set `spec` to {{target_spec_path}}, fill `location`, `library` (if applicable), and describe `primary_language` plus optional `secondary_languages`, replacing the references list with {{reference_items}}.
2. Summarize architecture and intent in Overview, explain language details, References, Implementation Details, API Surface, Data Models, and Operational Notes with concise prose linked to specification concepts.
3. Provide API signatures in fenced code blocks with notes on inputs, outputs, and side effects, removing HTML comment directives only after their guidance has been satisfied.
4. Validate against `spec/specman-data-model/spec.md` sections for Implementations, Implementing Language, References, APIs, and Implementation Metadata, then return Markdown ready for `impl/{{implementation_name}}/impl.md`.
