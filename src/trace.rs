// Work out the set of supported algorithms:
// * Fail at first byte.
// * Seek back up to N.
// * Miss encoding a location, and encode the next one.
// * Skip further back to find a longer code. Maintain the code lengths lengths?
// decode the thing symbol by symbol
// If a mode's decision isn't taken, drop that mode from the possible set of modes.
// If no modes are left, we didn't work.
// If any modes are left, pick the "simplest", and return it.

// Still need to fully decode the input, and store the whole backref search buffer.
// Can we use the same buffer? Probably too complex for first pass.

// Do we need to rearrange the api so we can process a sequence and its decoded bytes?
