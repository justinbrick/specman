# Compliance Tracking

You are verifying compliance for {{target_path}}.

## Steps

1. Retrieve the compliance report from `{{target_path}}/compliance`.
2. If any constraints are NOT covered, follow the Tool Calls below.

## Tool Calls

Only once the compliance report is retrieved and you have determined which constraints (if any) are not covered, run the following tool calls:

1. Call `create_refactor` to create a refactor scratch pad for the implementation.
2. Detail all constraint groups that are not assured.
3. Provide instructions on how to create the necessary tests or checks.
4. Provide instructions on how to add the required Validation Tags to confirm compliance in the next iteration.
