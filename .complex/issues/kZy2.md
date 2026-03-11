## Bug

The CLAUDE.md docs state "All mutation commands accept an optional `--reason` flag" but `cx block` does not support it:

```
$ cx block YhJD ZRAq --reason "read-only flag extends the multi-mount syntax"
error: unexpected argument '--reason' found
```

## Expected

`--reason` should work on `block` (and `unblock`, if it exists) the same way it works on `claim`, `shadow`, `integrate`, etc.

## Filed by

seguro:claude
