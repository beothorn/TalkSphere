# Exact numeric control fields

uint64 accepts only canonical unsigned decimal strings, with zero represented as
0 and all positive values without signs or leading zeros; overflow above uint64
is Invalid. uint32 accepts an exact JSON integer number token without fraction,
exponent or negative sign and checks the declared width. Neither passes through
binary64. These helpers perform representation validation, not price arithmetic
or economic authorization. Subsequent service scalar binding extends this file.
