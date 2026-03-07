# TODO

- ref(specman-mcp): add auto-completion for implementations, scratch pads, specification names.

- rev(specman-core): clarify documentation requirements
right now, the spec has requirements that the API documents it's own surface. I think this is fine for some areas, but really the impl docs should serve multi-purpose, it's meant to be a one-stop shop for user readable instructions, and maybe the details on the implementations should be built-in on the source code itself. it is the source of truth, after all. to fix this, need to think of a plan to revise wording so that impls aren't strictly "required" to document their interfaces, as it's largely redundant.
