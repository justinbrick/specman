# TODO

- rev(specman-cli): compliance tracking command
- fix(specman-mcp): resources do not get read from prompts
  - this might be a hard fix. right now, in vscode's implementation, even though resources are mention explicitly by handle, the AI does not read the resource content unless the user explicitly mentions it. this might be worth creating an issue on their github for, as this is not my fault, as far as i am aware.
- feat(specman-cli-rust): compliance tracking implementation
- rev(specman-core): add requirement to provide endpoint for checking workspace status
  - includes checking all of the specs + implementations:
  - ensures references are valid (references should include inline links, dependencies, references, and the location of the source code for implementations)
  - ensures no cyclic dependencies
- ref(specman-library): remove ops-based updating / editing
- feat(specman-cli): list constraints
- feat(specman-cli): read constraint content
