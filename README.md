# rezip

Rezip would like to be able to take a `DEFLATE` stream (e.g. a `.gz`/gzip
archive, or a `.zip` file), and produce a (small) metadata file about how
to recreate that exact file given the uncompressed data.

[gzip and deflate](gzip.md) are simple. However, it turns out, a lot of
existing compressors do weird things.

## Existing compressors

`gzip(1)`'s "lazy" optimisation works by omitting a match if it's immediately
followed by a longer match, which can work well in practice, e.g.
`a123412f41234`; when seeing the `412` following the `f`, you could insert a
backreference, but, if you do, you'll miss the proceeding `1234` backreference.

`gzip(1)` has a number of odd cases:

 * Won't use the first byte of the file for a reference, so can't compress
    `abcabc` into `(abc)\1`, but can convert `Sabcabc` into `S(abc)\1`.
 * Sometimes skips length-3 runs. It's unclear to me exactly when it does this.
 * Frequently inserts (harmless) garbage at the end of files.
 * Doesn't maintain enough state to emit sensible guesses in some cases, so you
   can trick it into omitting a match for a crafted data file. My favourite is
   `a3456789a23456a123412f4123456789`, where it will omit the backreference for
   the `412` following the `f`.

In the real world, I've also seen [Google Zopfli](https://github.com/google/zopfli),
which (intentionally) does some pretty weird things, and some Perl ecosystem
compressor, which does some *very* weird things.

In Rust land, a number of compressors implement what `gzip(1)` calls the "fast"
algorithm, where the lazy behaviour is not performed.

## Techniques

[pristine-tar](https://joeyh.name/code/pristine-tar/) does this, but in a
horrible, horrible way: it calls the system's `gzip(1)` with a whole range
of flags, and works out which output has the smallest binary diff from the input
file. This is ugly, and dependent on the system gzip compressor.

The aim for rezip is to have slow, but clear, code which can act as a reference
compressor, then deal with weird zip files with less efficient (but still
intelligent) formats. 

I've tried to implement various techniques. None have really worked out so far:

 * `emulate` attempts to be a bug-compatible gzip 1.6 implementation. This fails
    as apparently our sets of bugs don't line up.
 * `bestguess` generates all the options for an encoding at each point, ranks them,
    then stores which the original encoder picked. As we're pretty good at ranking,
    this should result in mostly a stream of zeros, which can then be compressed.
    However, it currently needs updating for the `lazy` behaviour, as otherwise
    real `gzip(1)` files require huge metadata.   
