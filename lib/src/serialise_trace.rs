use std::io;
use std::io::Read;
use std::u16;

use byteorder::LittleEndian as LE;
use byteorder::ReadBytesExt;
use byteorder::WriteBytesExt;

use itertools::Itertools;

use errors::*;

use Ref;
use Trace;
use u16_from;
use usize_from;

pub fn verify(traces: &[Trace]) -> Vec<u8> {
    let data = write(traces);
    assert_eq!(
        traces,
        read(io::Cursor::new(&data)).unwrap().as_slice(),
        "{:?}",
        data
    );
    data
}

pub fn write(traces: &[Trace]) -> Vec<u8> {
    let mut ret = Vec::with_capacity(traces.len());
    let mut traces = traces.into_iter().peekable();

    while let Some(&&trace) = traces.peek() {
        match trace {
            Trace::ActuallyLiteral => {
                ret.write_u16::<LE>(0).expect("writing to vector");
                traces.next();
            }
            Trace::Actually(r) => {
                ret.write_u16::<LE>(r.dist).expect("writing to vector");
                ret.push((r.run() - 3) as u8);
                traces.next();
            }
            Trace::Correct => {
                let mut corrects = traces.peeking_take_while(|x| Trace::Correct == **x).count();
                let representation_offset = 32_768;
                let max_representable = u16::MAX - representation_offset;
                while corrects > usize_from(max_representable) {
                    ret.write_u16::<LE>(representation_offset + max_representable)
                        .expect("writing to a vector");
                    corrects -= usize_from(max_representable);
                }

                assert_ne!(0, corrects);

                ret.write_u16::<LE>(representation_offset + u16_from(corrects))
                    .expect("writing to a vector");
            }
        }
    }

    ret
}

pub fn read<R: Read>(mut data: R) -> Result<Vec<Trace>> {
    let mut ret = Vec::new();

    loop {
        let first = match data.read_u16::<LE>() {
            Ok(first) => first,
            Err(ref e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => bail!(e),
        };

        if 0 == first {
            ret.push(Trace::ActuallyLiteral);
        } else if first <= 32_768 {
            let dist = first;
            let run_minus_3 = data.read_u8()?;
            ret.push(Trace::Actually(Ref::new(dist, u16::from(run_minus_3) + 3)));
        } else {
            let count = first - 32_768;
            for _ in 0..count {
                ret.push(Trace::Correct);
            }
        }
    }

    Ok(ret)
}

#[cfg(test)]
mod tests {
    use std::io;
    use super::Trace;

    fn assert_round_trip(trace: &[Trace]) {
        assert_eq!(
            trace,
            super::read(io::Cursor::new(super::write(trace)))
                .unwrap()
                .as_slice()
        );
    }

    #[test]
    fn long_trace() {
        let mut v = vec![Trace::Correct; 32_765];
        assert_round_trip(&v);
        v[1] = Trace::ActuallyLiteral;
        assert_round_trip(&v);
        v.push(Trace::ActuallyLiteral);
        assert_round_trip(&v);
        for _ in 0..10 {
            v.insert(5, Trace::Correct);
            assert_round_trip(&v);
        }
    }
}
