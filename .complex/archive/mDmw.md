cx claim with a nonexistent task ID returns "claimed" without error, but
the node never appears in cx tree or cx list. Silent success on invalid input.

Observed: worker-2 ran `cx claim H4WS.utLy.rmtg --as worker-2` — rmtg didn't
exist, cx said "claimed", worker-2 proceeded thinking it had a valid task.
It then created rmtg manually to work around the inconsistency.

Expected: cx claim on a nonexistent ID should return a non-zero exit code
and print an error message.

This is a bug in complex (cx) itself, not ox.
