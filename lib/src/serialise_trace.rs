use std::io;
use std::io::Read;

use byteorder::LittleEndian as LE;
use byteorder::ReadBytesExt;
use byteorder::WriteBytesExt;

use itertools::Itertools;

use errors::*;

use Code;
use Ref;
use Trace;
use u16_from;

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

    loop {
        let trace = match traces.peek() {
            Some(trace) => **trace,
            None => break,
        };

        match trace {
            Trace::ActuallyLiteral => {
                ret.write_u16::<LE>(0).expect("writing to vector");
                traces.next();
            },
            Trace::Actually(r) => {
                ret.write_u16::<LE>(r.dist).expect("writing to vector");
                ret.push((r.run() - 3) as u8);
                traces.next();
            },
            Trace::Correct => {
                let mut corrects = traces.peeking_take_while(|x| Trace::Correct == **x).count();
                while corrects > 32768 {
                    ret.write_u16::<LE>(32768).expect("writing to a vector");
                    corrects -= 32768;
                }

                ret.write_u16::<LE>(32768 + u16_from(corrects))
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
        } else if first <= 32768 {
            let dist = first;
            let run_minus_3 = data.read_u8()?;
            ret.push(Trace::Actually(Ref::new(
                dist,
                u16::from(run_minus_3 + 3),
            )));
        } else {
            let count = first - 32768;
            for _ in 0..count {
                ret.push(Trace::Correct);
            }
        }
    }

    Ok(ret)
}
