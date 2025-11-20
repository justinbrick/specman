# TODO

1. Add CLI

## Desirables

The system should ideally track unit tests that should be made via looking at the constraints (SHOULD, MUST, MAY, etc.)
This could be a separate pass telling the AI to look for these keywords, decide what's worth testing, then creating the tests.

Ideal steps for creating implementation information:
- Break concepts / key entities into sections
- Order concepts / key entities by dependency, so that the first thing required is at the top, and the last at the bottom.
- For each concept / key entity, create a file hierarchy detailing what files they should be located in.
- In each API section, reference the file that they should be stored in.

Other details required for scratch pads:
- Scratch pads should contain a note stating what the purpose of the scratch pad is.
    - This is to make sure that an AI can resume with little to no context. Each intro note should be well-formed enough for a cheaper LLM to pick up.
- Scratch pads should have a task list, detailing what's been currently done in the scratch pad, referencing files as needed.