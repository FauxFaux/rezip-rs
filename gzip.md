# What are gzip and zip?

`gzip` (`.gz`) and `zip` (`.zip`) are file formats encapsulating `DEFLATE`
streams. `zip` contains multiple independent streams. `gzip` contains just one.

The `gzip` file format is:

 * `10 byte header`, including a magic number, some flags, a timestamps, ...
 * Some extra header bytes, for e.g. an embedded file name.
 * A single `DEFLATE` stream.
 * `8 byte footer`, including a checksum and the file length.

That's it. [The `gzip` specification](https://tools.ietf.org/html/rfc1952) is
under five pages, including listings for all the possible options for every
flag, and discussion on what their meanings are.

Note that neither the header nor the footer are necessary (or even all that
useful) when decoding the data: everything of value is in the `DEFLATE` stream.

## What is `DEFLATE`?

`DEFLATE` seems complex due to its layers, but it's essentially very simple
underneath all of the obfuscation.

A `DEFLATE` stream consists of a list of frames or blocks, which have:

 * a boolean indicating whether this is the last frame
 * a type field for this frame

There are two types of frame. The first, `uncompressed`, means the frame
consists of just a length, then some bytes, which are taken literally.

The second is `huffman`. A `huffman` frame may have `dynamic` or `fixed`
`trees`. The format for a `huffman` frame is:

 * the `trees`, iff this is a `dynamic` frame.
 * a `huffman stream`

`trees` are a mapping from "lists of bits" to "opcodes", like:

| bits  |    opcode   |
| ----- | ----------- |
|  `01` | literal `a` |
| `101` | literal `b` |

The encoding is carefully designed to not be ambiguous, even with variable
length bit sequences. If you see `01101`, you know that it's [literal `a`,
literal `b`]; it cannot be anything else. Normally the codes are a lot longer
than in this trivial example.

The way these trees are built and stored in the file is complicated, and not
relevant for understanding the format. The compression scheme would work almost
as well if these trees were  written out literally, in instead of stored in the
crazy format.

The `fixed` tree is a single pre-agreed tree that all `DEFLATE` implementations
know, so you don't need to put it in the file. You can look at the `fixed`
tree in `huffman-fixed.txt`.

The available "opcodes" are:

 * literal single character, e.g. "literal `q`" or "literal `0x00`".
 * look back `b` bytes, and copy `r` bytes. I call this `reference`.
   * b can be up to 32k. r can be between 3 and 258.
 * end of block marker

That's it. That's all you can have in a file.

### Decoding through all the layers

Let's compress the unicode snowman, `☃`:

```
$ printf ☃ | gzip | xxd -b -c 1 | sed -n '11,15p'
0000000a: 01111011  {
0000000b: 00110100  4
0000000c: 10100011  .
0000000d: 00011001  .
0000000e: 00000000  .
```

This shows us the bitstream, without the header and footer.
Unfortunately, `xxd` outputs the bits in the "wrong" order, so you have
to read right-to-left, top-to-bottom (`1101_1110 0010_1100 110001...`).

So, the bit stream is:

 * `1`: this is the last block in the sequence
 * `10`: this is a `fixed` `huffman` block, so there's no following tree.
 * `111100010`: This code can be looked up in `fixed-huffman.txt`,
    which explains that it maps to the literal byte `0xe2`.
 * `110011000`: Also a fixed huffman code: literal byte `0x98`.
 * `10110011`: Also a fixed huffman code: literal byte `0x83`.
 * `0000000`: Also a fixed huffman code, this time for the end of stream marker,
    which means we should stop reading now.
 * `0000`: padding back up to a byte boundary, at the end of the stream.

So, our data is 0xe2, 0x98, 0x83:

```
$ printf '\xe2\x98\x83'
☃
```

The dump utility can automate this process:

```
$ printf ☃ | gzip | cargo run --example dump /dev/stdin                                                                                                                                  master  [ 15:21 ]
block 0:
 - fixed huffman:
    - lit: 0xe2: 'â'
    - lit: 0x98: '\u{98}'
    - lit: 0x83: '\u{83}'
```

Unfortunately, due to the (3+7+4=) 14 bit overhead, and using nine bits to
represent two of the three eight bit values, this hasn't actually reduced the
size of the snowman. The snowman is sad.


### Actual size reduction?

Let's look at a slightly more interesting example:

```
$ printf 'hello mellow jello!' | gzip | cargo run --example dump /dev/stdin
block 0:
 - fixed huffman:
    - lit: 0x68: 'h'
    - lit: 0x65: 'e'
    - lit: 0x6c: 'l'
    - lit: 0x6c: 'l'
    - lit: 0x6f: 'o'
    - lit: 0x20: ' '
    - lit: 0x6d: 'm'
    - backref: 6 byte(s) back, 4 bytes long
    - lit: 0x77: 'w'
    - lit: 0x20: ' '
    - lit: 0x6a: 'j'
    - backref: 7 byte(s) back, 4 bytes long
    - lit: 0x21: '!'
```

Here, the compressor has noticed that `hello`, `mellow` and `jello` share
`ello`, and has inserted backwards `reference`s to the previous place that
appeared in the stream: `6` bytes before the `ello` in `mellow`, there's
`4` interesting bytes in the middle of `hello`.

This requires 16 bytes to represent in `DEFLATE` (not in gzip, which is longer
due to the headers), which is shorter than the 19 byte input. We have a saving!

The savings in real files are a lot more notable; most files are composed of
a surprising number of back-references, if compressed by a competent compressor.

This file, for example, after compression, contains only around 1,100 literals,
and around 700 back-references, to represent over 5,000 bytes of data.
