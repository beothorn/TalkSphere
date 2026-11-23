# Canonical paths

A `CanonicalPath` names one resource within a page: the root `/`, or a slash followed by one or more components separated by single slashes with no trailing slash. A component is 1 to 255 UTF-8 bytes, is not `.` or
`..`, and has no control character (exactly U+0000 to U+001F and U+007F; the C1 controls U+0080 to U+009F are ordinary characters), no slash and no reverse solidus. `parse(bytes)` accepts path bytes only when they are valid shortest-form UTF-8 (no surrogates or overlong
forms) and a canonical path, otherwise `Invalid`. Paths are protocol strings: percent signs, question marks and number signs are literal, nothing is decoded or normalized, and case or Unicode normalization differences
make different names. `bytes`, `size` (compare with an offering's `maximumPathBytes`), `is_root`, `as_str`. Paths order by their bytes, which the manifest relies on.
