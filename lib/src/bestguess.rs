#[cfg(old_tests)]
mod tests {
    use circles::CircularBuffer;
    use serialise;
    use Code;
    use Code::Literal as L;
    use Ref;

    fn r(dist: u16, run: u16) -> Code {
        Code::Reference(Ref::new(dist, run))
    }

    #[test]
    fn re_1_single_backref_abcdef_bcdefghi() {
        let exp = &[
            L(b'a'),
            L(b'b'),
            L(b'c'),
            L(b'd'),
            L(b'e'),
            L(b'f'),
            L(b' '),
            r(6, 2 + 3),
            L(b'g'),
            L(b'h'),
            L(b'i'),
        ];

        assert_eq!(vec![0], decode_then_reencode_single_block(exp));
    }

    #[test]
    fn re_2_two_length_three_runs() {
        let exp = &[
            L(b'a'),
            L(b'b'),
            L(b'c'),
            L(b'd'),
            L(b'1'),
            L(b'2'),
            L(b'3'),
            L(b'e'),
            L(b'f'),
            L(b'g'),
            L(b'h'),
            L(b'7'),
            L(b'8'),
            L(b'9'),
            L(b'i'),
            L(b'j'),
            L(b'k'),
            L(b'l'),
            r(14, 0 + 3),
            L(b'm'),
            L(b'n'),
            L(b'o'),
            L(b'p'),
            r(14, 0 + 3),
            L(b'q'),
            L(b'r'),
            L(b's'),
            L(b't'),
        ];

        assert_eq!(vec![0, 0], decode_then_reencode_single_block(exp));
    }

    #[test]
    fn re_3_two_overlapping_runs() {
        let exp = &[
            L(b'a'),
            L(b'1'),
            L(b'2'),
            L(b'3'),
            L(b'b'),
            L(b'c'),
            L(b'd'),
            r(6, 0 + 3),
            L(b'4'),
            L(b'5'),
            L(b'e'),
            L(b'f'),
            r(5, 0 + 3),
            L(b'g'),
        ];

        assert_eq!(vec![0, 0], decode_then_reencode_single_block(exp));
    }

    #[test]
    fn re_4_zero_run() {
        let exp = &[L(b'0'), r(1, 10 + 3)];
        assert_eq!(vec![0], decode_then_reencode_single_block(exp));
    }

    #[test]
    fn re_5_ref_before() {
        let exp = &[r(1, 13)];
        assert_eq!(
            exp.iter().map(|_| 0usize).collect::<Vec<usize>>(),
            decode_maybe(&[0], exp)
        );
    }

    #[test]
    fn re_11_ref_long_before() {
        let exp = &[L(b'a'), L(b'b'), L(b'c'), L(b'd'), r(7, 13)];
        assert_eq!(
            &[0],
            decode_maybe(&[b'q', b'r', b's', b't', b'u'], exp).as_slice()
        );
    }

    #[test]
    fn re_12_ref_over_edge() {
        let exp = &[L(b'd'), r(2, 3)];
        assert_eq!(&[0], decode_maybe(&[b's', b't', b'u'], exp).as_slice());
    }

    #[test]
    fn re_6_just_long_run() {
        let exp = &[L(5), r(1, 258)];

        assert_eq!(vec![0], decode_then_reencode_single_block(exp));
    }

    #[test]
    fn re_7_two_long_run() {
        let exp = &[L(5), r(1, 258), r(1, 258)];

        assert_eq!(vec![0, 0], decode_then_reencode_single_block(exp));
    }

    #[test]
    fn re_8_many_long_run() {
        const ENOUGH_TO_WRAP_AROUND: usize = 10 + (32 * 1024 / 258);

        let mut exp = Vec::with_capacity(ENOUGH_TO_WRAP_AROUND + 1);

        exp.push(L(5));

        exp.extend(vec![r(1, 258); ENOUGH_TO_WRAP_AROUND]);

        assert_eq!(vec![0; 137], decode_then_reencode_single_block(&exp));
    }

    #[test]
    fn re_9_longer_match() {
        // I didn't think it would, but even:
        // echo a12341231234 | gzip --fast | cargo run --example dump /dev/stdin
        // ..generates this.

        // I was expecting it to only use the most recent hit for that hash item. Um.

        let exp = &[
            L(b'a'),
            L(b'1'),
            L(b'2'),
            L(b'3'),
            L(b'4'),
            r(4, 3),
            r(7, 4),
        ];

        assert_eq!(vec![0, 0], decode_then_reencode_single_block(exp));
    }

    fn decode_then_reencode_single_block(codes: &[Code]) -> Vec<usize> {
        decode_maybe(&[], codes)
    }

    fn decode_maybe(preroll: &[u8], codes: &[Code]) -> Vec<usize> {
        let mut data = Vec::with_capacity(codes.len());
        {
            let mut prebuf = CircularBuffer::with_capacity(32 * 1024);
            prebuf.extend(preroll);
            serialise::decompressed_codes(&mut data, &mut prebuf, codes).unwrap();
        }

        #[cfg(never)]
        println!(
            "data: {:?}, str: {:?}",
            data,
            String::from_utf8_lossy(&data)
        );

        let reduced = reduce_entropy(preroll, &data, codes).unwrap();
        assert_eq!(codes, increase_entropy(preroll, &data, &reduced).as_slice());
        reduced
    }

    #[test]
    fn short_repeat() {
        // a122b122222
        // 01234567890

        let exp = &[L(b'a'), r(1, 3)];

        assert_eq!(vec![0], decode_then_reencode_single_block(exp));
    }

    #[test]
    fn re_10_repeat_after_ref_a122b_122_222() {
        // a122b122222
        // 01234567890

        let exp = &[
            L(b'a'),
            L(b'1'),
            L(b'2'),
            L(b'2'),
            L(b'b'),
            r(4, 3),
            r(1, 3),
        ];

        assert_eq!(vec![0, 0], decode_then_reencode_single_block(exp));
    }

    #[test]
    fn lazy_longer_ref() {
        // Finally, a test for this gzip behaviour.
        // It only does this with zip levels >3, including the default.

        // a123412f41234
        // 0123456789012

        // It gets to position 8, and it's ignoring the "412" (at position 6),
        // instead taking the longer run of "1234" at position 1.

        // I bet it thinks it's so smart.

        let exp = &[
            L(b'a'),
            L(b'1'),
            L(b'2'),
            L(b'3'),
            L(b'4'),
            L(b'1'),
            L(b'2'),
            L(b'f'),
            L(b'4'),
            r(8, 4),
        ];

        assert_eq!(&[1, 0], decode_then_reencode_single_block(exp).as_slice());
    }

    #[test]
    fn long_prelude() {
        let exp = &[L(b'b'), r(3, 3)];

        let pre = concat(&[b'|'; 32768 + 1], b"ponies");

        assert_eq!(&[0], decode_maybe(&pre, exp).as_slice());
    }

    fn concat(x: &[u8], y: &[u8]) -> Box<[u8]> {
        let mut v = Vec::with_capacity(x.len() + y.len());
        v.extend(x);
        v.extend(y);
        v.into_boxed_slice()
    }
}
