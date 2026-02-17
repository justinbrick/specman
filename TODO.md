# TODO

- ref(specman-library): implement revisions made to specman-core for workspace status
  - [] plan API design. must be unified API, allows the user to provide options on if they would like to opt-out of specific checks.
  - [] includes checking all of the specs + implementations
  - [] ensures references are valid (references should include inline links, dependencies, references, and the location of the source code for implementations)
  - [] ensures no cyclic dependencies
  - [] valid frontmatter
- ref(specman-cli): use the new specman-library surface for workspace status
